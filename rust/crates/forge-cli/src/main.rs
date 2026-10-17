//! CLI de OpenBlock en Rust.
//!
//! El núcleo (`forge-core`) construye contenido y no toca el sistema; aquí vive
//! todo lo que sí lo toca: la detección de hardware y la escritura a disco. Es el
//! mismo reparto que el TS hace con `output.ts`.
//!
//! Subcomandos:
//!
//! - `forge detect [--json]` — hardware detectado, con el mismo JSON que el TS.
//! - `forge dump <config.json> <plan.json> <dir> [cores]` — proyecto a disco.
//!
//! En construcción: la resolución del plan contra las APIs (PaperMC/Modrinth) y
//! el doctor todavía no están portados.

mod detect;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn uso() -> ExitCode {
    eprintln!("uso: forge <comando> [opciones]");
    eprintln!();
    eprintln!("  detect [--json]                 hardware detectado (JSON igual que el TS)");
    eprintln!("  dump <config> <plan> <dir> [n]  genera el proyecto desde config y plan");
    eprintln!();
    eprintln!("El plan se obtiene del CLI de TypeScript: `openblock generate` lo escribe.");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("detect") => detect_cmd(&args[1..]),
        Some("dump") => dump_cmd(&args[1..]),
        _ => uso(),
    }
}

fn detect_cmd(args: &[String]) -> ExitCode {
    // `--raw-bytes` lo usa `tests/detect.test.ts` para comparar la detección de
    // Rust contra `os.totalmem()`/`os.release()` del TS sin depender de medidas
    // instantáneas, que cambian entre dos ejecuciones sin que nada esté mal.
    if args.iter().any(|a| a == "--raw-bytes") {
        let crudo = detect::bytes_crudos();
        match serde_json::to_string_pretty(&crudo) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("No se pudo serializar: {e}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }

    let perfil = detect::detect_hardware();
    if args.iter().any(|a| a == "--json") {
        match serde_json::to_string_pretty(&perfil) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("No se pudo serializar el perfil: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let tier = forge_core::hw::tiers::classify_hardware(&perfil);
        let ram = forge_core::hw::tiers::recommended_ram_mb(&perfil, true);
        let gc = forge_core::hw::tiers::recommended_gc(&perfil);
        println!("== Hardware ==");
        println!(
            "  Host        {} ({} {}, {})",
            perfil.hostname, perfil.os, perfil.os_version, perfil.arch
        );
        println!(
            "  CPU         {} — {} hilos",
            perfil.cpu_model, perfil.cores
        );
        println!(
            "  RAM         {} GB (libres {} GB) + swap {} GB",
            perfil.ram_total_mb / 1024,
            perfil.ram_free_mb / 1024,
            perfil.swap_total_mb / 1024
        );
        println!(
            "  Disco       {} (libres {} GB)",
            perfil.disk_kind.as_str(),
            perfil.disk_free_mb / 1024
        );
        println!(
            "  Tier        {} — hasta {} jugadores",
            tier.label, tier.max_players
        );
        println!(
            "  Recomendado {}, {}, vd {}/{}, -Xmx ~{ram} MB",
            tier.recommended_engine,
            gc.to_uppercase(),
            tier.view_distance,
            tier.simulation_distance
        );
    }
    ExitCode::SUCCESS
}

fn dump_cmd(args: &[String]) -> ExitCode {
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
