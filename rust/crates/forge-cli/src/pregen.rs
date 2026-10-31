//! `forge pregen <dir>` — pregenerar mundo con progreso real.
//!
//! El install ya pregenera al final, pero a ciegas y solo una vez. Esto es el
//! comando suelto: arranca la pregen de Chunky por RCON contra el server vivo
//! y lee el porcentaje que Chunky va escribiendo en el log —que es la única
//! forma honesta de medirlo, porque el RCON de Chunky no expone progreso.

use crate::rcon::with_rcon;
use crate::status::{cargar_cfg, puerto_abierto};
use crate::{resolver, ExitCode};
use std::fs;
use std::io::Seek;
use std::time::{Duration, Instant};

pub fn pregen_cmd(args: &[String]) -> ExitCode {
    let mut dir: Option<&String> = None;
    let mut radio: Option<u32> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--radius" if i + 1 < args.len() => {
                radio = args[i + 1].parse().ok();
                i += 2;
            }
            otro if !otro.starts_with('-') && dir.is_none() => {
                dir = Some(&args[i]);
                i += 1;
            }
            _ => i += 1,
        }
    }
    let Some(dir) = dir else {
        eprintln!("uso: forge pregen <dir> [--radius N]");
        eprintln!("  el server tiene que estar arriba (start.sh/.bat) y llevar Chunky");
        return ExitCode::from(2);
    };
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let radio = radio.unwrap_or(if cfg.pregen_radius > 0 {
        cfg.pregen_radius
    } else {
        2000
    });
    if !puerto_abierto(cfg.port) {
        eprintln!(
            "El server está caído (puerto {}): arráncalo y vuelve",
            cfg.port
        );
        return ExitCode::FAILURE;
    }
    if !cfg.rcon.enabled {
        eprintln!("RCON deshabilitado: la pregen se lanza por RCON");
        return ExitCode::FAILURE;
    }

    let log = resolver(dir).join("logs").join("latest.log");
    let marca = match fs::metadata(&log) {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    let lanzar = |r: &mut crate::rcon::RconClient| -> Result<String, crate::rcon::Error> {
        r.command(&format!("chunky radius {radio}"))?;
        r.command("chunky start")?;
        Ok(String::new())
    };
    if let Err(e) = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, lanzar) {
        eprintln!("No se pudo lanzar la pregen: {e}");
        eprintln!("  ¿Chunky está instalado? forge plugins {dir} add chunky");
        return ExitCode::FAILURE;
    }
    println!("Pregeneración lanzada (radio {radio}). Progreso (Ctrl+C para dejar de mirar, la tarea sigue):");

    let inicio = Instant::now();
    if let Ok(mut fh) = fs::File::open(&log) {
        fh.seek(std::io::SeekFrom::Start(marca)).ok();
        let mut buf = vec![0u8; 16384];
        use std::io::Read;
        loop {
            match fh.read(&mut buf) {
                Ok(0) => std::thread::sleep(Duration::from_millis(700)),
                Ok(n) => {
                    for linea in String::from_utf8_lossy(&buf[..n]).split('\n') {
                        let l = linea.trim();
                        if let Some(pct) = porcentaje(l) {
                            eprint!("\r  {:>3}%  ({:>5} s)  ", pct, inicio.elapsed().as_secs());
                        }
                        if l.contains("Task finished") {
                            println!("\n✓ Pregeneración terminada.");
                            return ExitCode::SUCCESS;
                        }
                    }
                }
                Err(_) => std::thread::sleep(Duration::from_millis(700)),
            }
        }
    }
    ExitCode::SUCCESS
}

/// Chunky escribe líneas tipo "Task running for world. 123/456 chunks (27%)"
fn porcentaje(linea: &str) -> Option<u32> {
    let i = linea.find('(')?;
    let resto = &linea[i + 1..];
    let fin = resto.find('%')?;
    resto[..fin].trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::porcentaje;

    #[test]
    fn saca_el_porcentaje_de_las_lineas_de_chunky() {
        assert_eq!(
            porcentaje("Task running for world. 123/456 chunks (27%)"),
            Some(27)
        );
        assert_eq!(
            porcentaje("Task running for world. 1/456 chunks (0%)"),
            Some(0)
        );
        assert_eq!(porcentaje("Task finished for world. 456/456 chunks."), None);
        assert_eq!(porcentaje("sin porcentaje aqui"), None);
    }
}
