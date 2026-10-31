//! `forge status <dir>` — ¿está vivo? ¿quién juega? ¿a qué TPS?
//!
//! Es lo primero que uno quiere saber de un server instalado, y hasta ahora
//! obligaba a leer el log o acordarse del puerto. Esto hace un sondeo TCP al
//! puerto del server y, si responde y hay RCON, saca jugadores y TPS (spark).
//! También enseña las IPs locales de la máquina: para entrar desde otra
//! consola/LAN hace falta, y averiguarlas a mano en Windows es un peaje.

use crate::rcon::with_rcon;
use crate::{resolver, ExitCode};
use forge_core::ServerConfig;
use std::fs;
use std::net::{TcpStream, UdpSocket};
use std::path::Path;
use std::time::Duration;

/// Carga la config de un server instalado (`openblock.json`).
///
/// El `server_dir` embebido puede ser de otro SO —el zip se genera en Windows y
/// se corre en Linux— así que si no existe como ruta local, se usa el `--dir`
/// actual. Es el mismo arreglo que ya hace `stop`.
pub(crate) fn cargar_cfg(dir: &str) -> Result<(ServerConfig, std::path::PathBuf), String> {
    let raiz = resolver(dir);
    let cfg_path = raiz.join("openblock.json");
    let cfg_txt = fs::read_to_string(&cfg_path).map_err(|_| {
        format!(
            "No existe {}: instala el server primero",
            cfg_path.display()
        )
    })?;
    let mut cfg: ServerConfig =
        serde_json::from_str(&cfg_txt).map_err(|e| format!("openblock.json inválido: {e}"))?;
    if !Path::new(&cfg.server_dir).exists() {
        cfg.server_dir = raiz.to_string_lossy().to_string();
    }
    Ok((cfg, raiz))
}

/// ¿Hay algo escuchando en el puerto? (sondeo TCP con timeout corto).
pub(crate) fn puerto_abierto(puerto: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{puerto}").parse().unwrap(),
        Duration::from_millis(1500),
    )
    .is_ok()
}

/// La IP local "hacia fuera", sin enviar nada: conectar un UDP no manda
/// paquetes, solo fija la ruta por defecto, y así sabemos qué IP usaría.
fn ip_local() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

pub fn status_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge status <dir>");
        return ExitCode::from(2);
    };
    let (cfg, _raiz) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let arriba = puerto_abierto(cfg.port);
    println!("== {} ({} {}) ==", cfg.name, cfg.engine, cfg.mc_version);
    if arriba {
        println!("Server:  ✓ arriba (puerto {})", cfg.port);
    } else {
        println!("Server:  ✗ caído (nada escucha en {})", cfg.port);
    }
    if let Some(ip) = ip_local() {
        println!(
            "Entrar:  {ip}:{}  (desde esta misma máquina: localhost:{})",
            cfg.port, cfg.port
        );
    } else {
        println!("Entrar:  localhost:{}", cfg.port);
    }

    if arriba && cfg.rcon.enabled {
        let r = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
            let lista = r.command("list").unwrap_or_default();
            let tps = r.command("spark tps").ok().filter(|s| !s.trim().is_empty());
            Ok((lista, tps))
        });
        match r {
            Ok((lista, tps)) => {
                if !lista.trim().is_empty() {
                    println!("Jugadores: {}", lista.trim());
                }
                match tps {
                    Some(t) => println!("TPS (spark):\n{t}"),
                    None => println!("TPS: sin spark instalado (`spark tps` no respondió)"),
                }
            }
            Err(e) => println!("RCON: {e} (¿contraseña cambiada en server.properties?)"),
        }
    }
    ExitCode::SUCCESS
}
