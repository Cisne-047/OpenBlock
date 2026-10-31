//! `forge list` — el registro de servers de esta máquina.
//!
//! Con un server, sobra saberse el directorio; con tres, no. Cada `install`
//! exitoso deja ficha en `~/.openblock/servers.json` y `forge list` las enseña
//! con el estado en vivo de cada una (sondeo de puerto, barato). `forge status
//! --all` es lo mismo con detalle. El registro nunca es requisito: si un
//! directorio tiene `openblock.json`, todos los comandos siguen funcionando
//! igual pasándole la ruta.

use crate::status::puerto_abierto;
use crate::ExitCode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Una ficha del registro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ficha {
    pub name: String,
    pub engine: String,
    pub mc_version: String,
    pub port: u16,
    /// directorio absoluto, la clave de verdad
    pub dir: String,
}

fn fichero() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".openblock").join("servers.json")
}

pub fn leer() -> Vec<Ficha> {
    let Ok(txt) = fs::read_to_string(fichero()) else {
        return Vec::new();
    };
    serde_json::from_str(&txt).unwrap_or_default()
}

fn escribir(fichas: &[Ficha]) -> Result<(), String> {
    let p = fichero();
    if let Some(padre) = p.parent() {
        fs::create_dir_all(padre).map_err(|e| e.to_string())?;
    }
    fs::write(&p, serde_json::to_string_pretty(fichas).unwrap() + "\n").map_err(|e| e.to_string())
}

/// Registra (o refresca) la ficha de un server recién instalado/actualizado.
/// Fallar no puede tumbar un install: se ignora y ya está.
pub fn registrar(cfg: &forge_core::ServerConfig, dir: &std::path::Path) {
    let mut fichas = leer();
    let dir_txt = dir.to_string_lossy().to_string();
    fichas.retain(|f| !fichas_rutas_iguales(&f.dir, &dir_txt));
    fichas.push(Ficha {
        name: cfg.name.clone(),
        engine: cfg.engine.to_string(),
        mc_version: cfg.mc_version.clone(),
        port: cfg.port,
        dir: dir_txt,
    });
    let _ = escribir(&fichas);
}

/// compara rutas sin depender del separador ni de la existencia
fn fichas_rutas_iguales(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm(a) == norm(b)
}

/// quita la ficha de un directorio (si el server se borra a mano, `forge list`
/// lo detecta solo porque la ruta ya no existe; esto es para el caso raro)
pub fn olvidar(dir: &str) -> bool {
    let mut fichas = leer();
    let antes = fichas.len();
    fichas.retain(|f| !fichas_rutas_iguales(&f.dir, dir));
    if fichas.len() == antes {
        return false;
    }
    escribir(&fichas).is_ok()
}

pub fn list_cmd(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "--forget") {
        let dir = args.iter().find(|a| !a.starts_with('-')).unwrap();
        if olvidar(dir) {
            println!("✓ {} fuera del registro", dir);
            return ExitCode::SUCCESS;
        }
        println!("• {} no estaba en el registro", dir);
        return ExitCode::SUCCESS;
    }
    let fichas: Vec<Ficha> = leer()
        .into_iter()
        .filter(|f| PathBuf::from(&f.dir).join("openblock.json").exists())
        .collect();
    if fichas.is_empty() {
        println!("No hay servers registrados (los que instales con forge quedan aquí).");
        println!(
            "  ¿instalado a mano con openblock.json? pásale el directorio a cualquier comando."
        );
        return ExitCode::SUCCESS;
    }
    println!(
        "{NOMBRE:<16} {ENGINE:<10} {MC:<8} {PORT:<7} ESTADO",
        NOMBRE = "NOMBRE",
        ENGINE = "ENGINE",
        MC = "MC",
        PORT = "PUERTO"
    );
    for f in &fichas {
        let estado = if puerto_abierto(f.port) {
            "arriba"
        } else {
            "caído"
        };
        println!(
            "{:<16} {:<10} {:<8} {:<7} {}",
            f.name, f.engine, f.mc_version, f.port, estado
        );
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{fichas_rutas_iguales, Ficha};

    #[test]
    fn rutas_con_separador_distinto_son_la_misma() {
        assert!(fichas_rutas_iguales(
            r"F:\Servers\mi-server",
            "f:/servers/mi-server/"
        ));
        assert!(!fichas_rutas_iguales("/a/b", "/a/c"));
    }

    #[test]
    fn fichas_serializan_con_todo() {
        let f = Ficha {
            name: "x".into(),
            engine: "paper".into(),
            mc_version: "1.21.4".into(),
            port: 25565,
            dir: "/tmp/x".into(),
        };
        let j = serde_json::to_string(&f).unwrap();
        assert!(j.contains("\"port\":25565"));
    }
}
