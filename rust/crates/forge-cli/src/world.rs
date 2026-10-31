//! `forge world` — info del mundo y poda de chunks lejanos.
//!
//! `info` enseña tamaños reales de los tres mundos, el seed y la dificultad
//! (de server.properties) y el borde si el server está arriba con ChunkyBorder.
//! `trim` encoge el mundo: fija el borde con ChunkyBorder y lanza el `chunky
//! trim`, que borra los chunks de fuera — menos disco, backups más pequeños y
//! RAM de mapas menor. Solo con el server vivo y Chunky+ChunkyBorder puestos.

use crate::rcon::with_rcon;
use crate::status::{cargar_cfg, puerto_abierto};
use crate::{resolver, ExitCode};
use std::fs;
use std::path::Path;

pub fn world_cmd(args: &[String]) -> ExitCode {
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if pos.len() < 2 {
        eprintln!("uso: forge world <dir> info | trim [--radius N]");
        return ExitCode::from(2);
    }
    let (dir, accion) = (pos[0], pos[1].as_str());
    let radio = flag_valor(args, "--radius").and_then(|r| r.parse::<u32>().ok());
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let raiz = resolver(dir);
    match accion {
        "info" => info(&cfg, &raiz),
        "trim" => trim(&cfg, &raiz, radio),
        otra => {
            eprintln!("acción desconocida: {otra} (info|trim)");
            ExitCode::from(2)
        }
    }
}

fn flag_valor<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn info(cfg: &forge_core::ServerConfig, raiz: &Path) -> ExitCode {
    println!("== mundo de {} ==", cfg.name);
    for props in leer_properties(&raiz.join("server.properties")) {
        match props.0.as_str() {
            "level-seed" if !props.1.is_empty() => println!("Seed:      {}", props.1),
            "difficulty" => println!("Dificultad: {}", props.1),
            "level-name" => println!("Nivel:     {}", props.1),
            _ => {}
        }
    }
    let mut total = 0u64;
    for mundo in ["world", "world_nether", "world_the_end"] {
        let p = raiz.join(mundo);
        if p.exists() {
            let b = tamano(&p);
            total += b;
            println!("  {mundo:<16} {:>12}", humano(b));
        }
    }
    println!("Total: {} (+ backups aparte)", humano(total));
    if puerto_abierto(cfg.port) && cfg.rcon.enabled {
        let borde = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
            r.command("chunkyborder list")
                .or_else(|_| r.command("chunky border"))
        });
        if let Ok(b) = borde {
            if !b.trim().is_empty() {
                println!("Borde: {}", b.trim());
            }
        }
    }
    ExitCode::SUCCESS
}

fn trim(cfg: &forge_core::ServerConfig, raiz: &Path, radio: Option<u32>) -> ExitCode {
    let Some(radio) = radio else {
        eprintln!("dime el radio del borde: forge world <dir> trim --radius 3000");
        eprintln!("  todo lo de fuera del borde se BORRA (chunks regenerables, construcciones no)");
        return ExitCode::from(2);
    };
    if !puerto_abierto(cfg.port) {
        eprintln!("El trim se lanza con el server vivo (Chunky + ChunkyBorder): arráncalo primero");
        return ExitCode::FAILURE;
    }
    if !cfg.rcon.enabled {
        eprintln!("RCON deshabilitado: el trim se lanza por RCON");
        return ExitCode::FAILURE;
    }
    let antes = tamano(&raiz.join("world"));
    let r = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
        let a = r.command(&format!("chunkyborder add {radio}"))?;
        r.command("chunky trim")?;
        Ok(a)
    });
    match r {
        Ok(_) => {
            println!("✓ Borde {radio} fijado y trim lanzado. El progreso sale en la consola/log del server.");
            println!(
                "  (espacio antes: {}; `forge world {} info` después para comparar)",
                humano(antes),
                raiz.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("No se pudo lanzar: {e}");
            eprintln!(
                "  ¿ChunkyBorder está? forge plugins {} add chunkyborder",
                raiz.display()
            );
            ExitCode::FAILURE
        }
    }
}

fn leer_properties(p: &Path) -> Vec<(String, String)> {
    let Ok(txt) = fs::read_to_string(p) else {
        return Vec::new();
    };
    txt.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') || !l.contains('=') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect()
}

/// bytes en KB/MB/GB legibles: un mundo de 300 KB no es "0.0 MB"
pub(crate) fn humano(b: u64) -> String {
    if b >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1024 * 1024 {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    } else {
        format!("{} KB", b / 1024)
    }
}

fn tamano(dir: &Path) -> u64 {
    fn rec(p: &Path) -> u64 {
        let mut n = 0;
        if let Ok(leidos) = fs::read_dir(p) {
            for e in leidos.flatten() {
                let m = e.metadata();
                n += match m {
                    Ok(m) if m.is_dir() => rec(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                };
            }
        }
        n
    }
    rec(dir)
}

#[cfg(test)]
mod tests {
    use super::{leer_properties, tamano};
    use std::fs;

    #[test]
    fn properties_se_leen_sin_comentarios() {
        let dir = std::env::temp_dir().join(format!("ob-world-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("server.properties");
        fs::write(&p, "# comentario\nlevel-seed=123\ndifficulty=hard\n").unwrap();
        let props = leer_properties(&p);
        assert!(props.contains(&("level-seed".into(), "123".into())));
        assert!(props.contains(&("difficulty".into(), "hard".into())));
        assert_eq!(props.len(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tamano_cuenta_recursivo() {
        let dir = std::env::temp_dir().join(format!("ob-world2-{}", std::process::id()));
        fs::create_dir_all(dir.join("region")).unwrap();
        fs::write(dir.join("a.dat"), vec![0u8; 1024]).unwrap();
        fs::write(dir.join("region").join("r.dat"), vec![0u8; 2048]).unwrap();
        assert_eq!(tamano(&dir), 3072);
        fs::remove_dir_all(&dir).ok();
    }
}
