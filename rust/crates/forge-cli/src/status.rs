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
    // --wait-healthy [segundos]: espera a que el puerto abra (para scripts/CI)
    if let Some(i) = args.iter().position(|a| a == "--wait-healthy") {
        let seg: u64 = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(180);
        return esperar_sano(args, seg);
    }
    // --webhook URL: manda un aviso (formato Discord) si algo no va — para
    // programarlo con cron/timer; no es un proceso residente
    if let Some(i) = args.iter().position(|a| a == "--webhook") {
        if let Some(url) = args.get(i + 1) {
            return alerta(args, url);
        }
        eprintln!("--webhook necesita la URL (formato Discord: el JSON {{content}})");
        return ExitCode::from(2);
    }
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge status <dir> [--wait-healthy [s]] [--webhook URL]");
        return ExitCode::from(2);
    };
    status_de(dir)
}

/// el cuerpo que era status_cmd
pub(crate) fn status_de(dir: &str) -> ExitCode {
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

/// `--wait-healthy [s]`: sondea hasta que el puerto abra (o el plazo venza).
///
/// Sale 0 si acaba arriba y 1 si no: pensado para scripts —esperar a que un
/// `start.sh` recién lanzado termine de arrancar antes de mandarle RCON.
fn esperar_sano(args: &[String], seg: u64) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge status <dir> --wait-healthy [segundos]");
        return ExitCode::from(2);
    };
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seg);
    print!("esperando a {} en el puerto {} …", dir, cfg.port);
    loop {
        if puerto_abierto(cfg.port) {
            println!("\n✓ arriba");
            return ExitCode::SUCCESS;
        }
        if std::time::Instant::now() > deadline {
            println!("\n✗ sin señal tras {seg} s");
            return ExitCode::FAILURE;
        }
        print!(".");
        use std::io::Write;
        std::io::stdout().flush().ok();
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
}

/// `--webhook URL`: mira el server y avisa (Discord y compatibles: POST JSON
/// `{"content": "…"}`) si está caído o con TPS < 15. Una pasada, sin bucle:
/// para vigilancia continua, un cron/timer que lo llame cada pocos minutos.
fn alerta(args: &[String], url: &str) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge status <dir> --webhook <URL>");
        return ExitCode::from(2);
    };
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mensaje = if !puerto_abierto(cfg.port) {
        Some(format!(
            "🔴 {} está CAÍDO (nada en el puerto {})",
            cfg.name, cfg.port
        ))
    } else if cfg.rcon.enabled {
        let tps = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
            r.command("spark tps")
        })
        .ok()
        .and_then(|s| primer_tps(&s));
        match tps {
            Some(t) if t < 15.0 => Some(format!("🟡 {} con TPS bajo: {t:.1}", cfg.name)),
            Some(_) => None, // sano: no molesta
            None => None,    // sin spark: solo el caído avisa
        }
    } else {
        None
    };
    let Some(msg) = mensaje else {
        println!("✓ {}: sin novedad (no se avisa)", cfg.name);
        return ExitCode::SUCCESS;
    };
    match postear(url, &msg) {
        Ok(()) => {
            println!("✓ avisado: {msg}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ el aviso no llegó: {e}");
            ExitCode::FAILURE
        }
    }
}

/// el primer número de TPS de la salida de spark ("TPS: 19.8 (5m: …)")
fn primer_tps(salida: &str) -> Option<f64> {
    let tras = salida.split("TPS").nth(1)?;
    let num: String = tras
        .trim_start_matches(|c: char| c != '-' && !c.is_ascii_digit() && c != '.')
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num.parse().ok()
}

/// POST JSON sin librería: curl ya es dependencia de facto del proyecto
fn postear(url: &str, contenido: &str) -> Result<(), String> {
    let cuerpo = serde_json::json!({ "content": contenido }).to_string();
    let out = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
        ])
        .arg(&cuerpo)
        .arg(url)
        .output()
        .map_err(|e| format!("lanzar curl: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::primer_tps;

    #[test]
    fn saca_el_primer_tps_de_spark() {
        assert_eq!(primer_tps("TPS: 19.87 (5m: 19.99)"), Some(19.87));
        assert_eq!(primer_tps("TPS: 12.5 (5m: 13)"), Some(12.5));
        assert_eq!(primer_tps("otra cosa"), None);
    }
}
