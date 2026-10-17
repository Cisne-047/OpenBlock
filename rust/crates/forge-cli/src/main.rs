//! CLI de OpenBlock en Rust.
//!
//! `forge dump <config.json> <plan.json> <dir> [cores]` — genera el proyecto y lo
//! escribe en disco. Es el mismo contrato que el `forge dump` de DistroForge: el
//! núcleo construye el `FileMap` y el CLI es el único que toca el disco.
//!
//! Está en construcción: todavía no resuelve el plan contra las APIs (eso es
//! `plan.rs`/`api/` del lado TS), así que el plan se le pasa ya resuelto.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn uso() -> ExitCode {
    eprintln!("uso: forge dump <config.json> <plan.json> <dir> [cores]");
    eprintln!();
    eprintln!("  Genera el proyecto del server desde una config y un plan ya resueltos.");
    eprintln!("  El plan se obtiene del CLI de TypeScript: `openblock generate` lo escribe.");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, resto) = match args.first().map(String::as_str) {
        Some("dump") => ("dump", &args[1..]),
        // sin subcomando se asume dump, como el `forge` de DistroForge
        _ => ("dump", &args[..]),
    };
    match cmd {
        "dump" => dump(resto),
        _ => uso(),
    }
}

fn dump(args: &[String]) -> ExitCode {
    if args.len() < 3 {
        return uso();
    }
    let cfg_path = Path::new(&args[0]);
    let plan_path = Path::new(&args[1]);
    let out_dir = Path::new(&args[2]);
    let cores: u32 = args
        .get(3)
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(num_cpus);

    let cfg_txt = match fs::read_to_string(cfg_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config inválida en {}: {e}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let plan_txt = match fs::read_to_string(plan_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", plan_path.display());
            return ExitCode::FAILURE;
        }
    };
    let plan: forge_core::plan::DownloadPlan = match serde_json::from_str(&plan_txt) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Plan inválido en {}: {e}", plan_path.display());
            return ExitCode::FAILURE;
        }
    };

    let errores = forge_core::generators::validate(&cfg);
    if !errores.is_empty() {
        eprintln!("Config inválida:");
        for e in &errores {
            eprintln!("  - {e}");
        }
        return ExitCode::FAILURE;
    }

    let files = forge_core::generators::generate_project(&cfg, &plan, cores);
    let mut escritos = 0usize;
    for (nombre, contenido) in &files {
        let destino = out_dir.join(nombre);
        if let Some(padre) = destino.parent() {
            if let Err(e) = fs::create_dir_all(padre) {
                eprintln!("No se pudo crear {}: {e}", padre.display());
                return ExitCode::FAILURE;
            }
        }
        // LF en todo; CRLF solo en .bat, igual que `writeProject` del TS
        let eol = if nombre.ends_with(".bat") {
            "\r\n"
        } else {
            "\n"
        };
        // el contenido puede llevar ya CRLF si alguien lo metió; aquí se normaliza
        let normalizado = contenido.replace("\r\n", "\n").replace('\n', eol);
        if let Err(e) = fs::write(&destino, normalizado) {
            eprintln!("No se pudo escribir {}: {e}", destino.display());
            return ExitCode::FAILURE;
        }
        escritos += 1;
    }

    println!(
        "Proyecto {} volcado en {} ({escritos} ficheros)",
        cfg.name,
        out_dir.display()
    );
    ExitCode::SUCCESS
}

/// Cores lógicos sin traer una dependencia: `available_parallelism` es de std.
fn num_cpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
}
