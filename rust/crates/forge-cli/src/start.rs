//! `forge start <dir>` y `forge watch <dir>` — arrancar y vigilar.
//!
//! `start` lanza el `start.sh`/`start.bat` del server desacoplado de esta
//! terminal (el bucle anti-crash vive en ese script, así que el arranque desde
//! aquí conserva el reinicio ante crash). La salida va a `logs/forge-boot.log`
//! para que nada se pierda. Es el par de `forge stop`, que ya existía.
//!
//! `watch` es el dashboard: cada 2 s limpia y repinta —arriba/caído, jugadores,
//! TPS de spark y las últimas líneas del log— hasta Ctrl+C. Sin dependencias
//! de TUI: ANSI pelado, que en una terminal de server es lo que hay.

use crate::rcon::with_rcon;
use crate::status::{cargar_cfg, puerto_abierto};
use crate::{resolver, ExitCode};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

/// Arranca el script del server, desacoplado. Devuelve el PID (Unix) o nada
/// (Windows, donde el PID es del cmd y no vale la pena prometerlo).
pub(crate) fn arrancar(raiz: &Path, log: &mut impl Write) -> Result<(), String> {
    if puerto_abierto_leer(raiz) {
        return Err("ya hay algo escuchando en el puerto del server".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED: u32 = 0x0000_0008; // DETACHED_PROCESS
        let logpath = raiz.join("logs").join("forge-boot.log");
        if let Some(p) = logpath.parent() {
            fs::create_dir_all(p).ok();
        }
        let hijo = Command::new("cmd")
            .args(["/c", "start.bat"])
            .current_dir(raiz)
            .creation_flags(DETACHED)
            .stdout(Stdio::from(
                File::create(&logpath).map_err(|e| e.to_string())?,
            ))
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("lanzar start.bat: {e}"))?;
        let _ =
            log.write_all(format!("cmd pid {} (start.bat desacoplado)\n", hijo.id()).as_bytes());
        // el return estructura los bloques cfg(windows/unix); allow al estilo
        #[allow(clippy::needless_return)]
        return Ok(());
    }
    #[cfg(unix)]
    {
        let logpath = raiz.join("logs").join("forge-boot.log");
        if let Some(p) = logpath.parent() {
            fs::create_dir_all(p).ok();
        }
        // nohup casero: nueva sesión, salida al log, y no esperamos
        let hijo = Command::new("bash")
            .arg("start.sh")
            .current_dir(raiz)
            .stdout(Stdio::from(
                File::create(&logpath).map_err(|e| e.to_string())?,
            ))
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("lanzar start.sh: {e}"))?;
        let _ = log.write_all(format!("pid {} (start.sh)\n", hijo.id()).as_bytes());
        // el hijo vive su vida: soltar sin wait
        std::mem::forget(hijo);
        Ok(())
    }
}

/// lee el puerto del openblock.json del directorio (para no pasar cfg por todos lados)
fn puerto_abierto_leer(raiz: &Path) -> bool {
    let Ok(txt) = fs::read_to_string(raiz.join("openblock.json")) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else {
        return false;
    };
    v["port"]
        .as_u64()
        .map(|p| puerto_abierto(p as u16))
        .unwrap_or(false)
}

pub fn start_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge start <dir>");
        eprintln!("  lanza start.sh/.bat desacoplado (bucle anti-crash incluido); par: forge stop");
        return ExitCode::from(2);
    };
    let raiz = resolver(dir);
    if !raiz.join("openblock.json").exists() {
        eprintln!("No hay openblock.json en {}", raiz.display());
        return ExitCode::FAILURE;
    }
    let script = if cfg!(windows) {
        "start.bat"
    } else {
        "start.sh"
    };
    if !raiz.join(script).exists() {
        eprintln!("Falta {script}: instala primero (forge install)");
        return ExitCode::FAILURE;
    }
    let mut log = std::io::stdout();
    match arrancar(&raiz, &mut log) {
        Ok(()) => {
            println!("✓ server arrancando (log: logs/forge-boot.log)");
            println!("  espera con: forge status {dir} --wait-healthy");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn watch_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge watch <dir>   (Ctrl+C para salir)");
        return ExitCode::from(2);
    };
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let raiz = resolver(dir);
    let log = raiz.join("logs").join("latest.log");
    let inicio = SystemTime::now();
    loop {
        let arriba = puerto_abierto(cfg.port);
        let (jugadores, tps) = if arriba && cfg.rcon.enabled {
            with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
                let j = r.command("list").unwrap_or_default();
                let t = r.command("spark tps").ok().filter(|s| !s.trim().is_empty());
                Ok((j, t))
            })
            .unwrap_or_default()
        } else {
            (String::new(), None)
        };
        // repintar
        print!("\x1b[2J\x1b[H");
        println!(
            "== {} ({} {}) == {}",
            cfg.name,
            cfg.engine,
            cfg.mc_version,
            hace(inicio)
        );
        println!("Server:   {}", if arriba { "✓ arriba" } else { "✗ caído" });
        if !jugadores.trim().is_empty() {
            println!("Jugadores: {}", jugadores.trim());
        }
        match tps {
            Some(t) => println!("TPS:\n{}", recorta_tps(&t)),
            None if arriba => println!("TPS: sin spark (forge plugins {} add spark)", dir),
            None => {}
        }
        if let Ok(lineas) = fs::read_to_string(&log) {
            let ultimas: Vec<&str> = lineas.lines().collect::<Vec<_>>()
                [lineas.lines().count().saturating_sub(6)..]
                .to_vec();
            println!("--- log (últimas) ---");
            for l in ultimas {
                println!("  {}", recorta(l, 100));
            }
        }
        println!("(Ctrl+C para salir · forge status {dir} para una foto fija)");
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// "hace 3 min" desde el arranque del watch
fn hace(desde: SystemTime) -> String {
    let s = SystemTime::now()
        .duration_since(desde)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if s < 60 {
        format!("({s}s)")
    } else {
        format!("({} min)", s / 60)
    }
}

/// spark tps suelta varias líneas y pestañea: la primera, que es la del ahora
fn recorta_tps(s: &str) -> String {
    s.lines().take(2).collect::<Vec<_>>().join("\n")
}

fn recorta(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let c: String = s.chars().take(max).collect();
        format!("{c}…")
    } else {
        s.to_string()
    }
}
