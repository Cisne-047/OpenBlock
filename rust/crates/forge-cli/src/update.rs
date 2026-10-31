//! `forge update <dir>` — subir de build/engine sin perder el mundo.
//!
//! Actualizar un server a mano es Bingo: bajar el jar nuevo, no borrar el
//! equivocado, no pisar `server.properties`. Esto re-resuelve el plan con la
//! config ya instalada (respetando `--build`/`--loader-build` si se fijaron),
//! baja solo el engine, reescribe los scripts/flags/README y borra el jar
//! viejo. **No toca** mundo, `server.properties`, eula, whitelist, ops,
//! plugins ni mods: eso es tuyo.
//!
//! Con forge/neoforge re-ejecuta el installer, que es quien trae las librerías
//! del loader nuevo.

use crate::status::{cargar_cfg, puerto_abierto};
use crate::{resolver, ExitCode};
use forge_core::generators::generate_project;
use forge_core::plan::DownloadKind;
use forge_core::EngineId;
use std::fs;
use std::path::Path;

pub fn update_cmd(args: &[String]) -> ExitCode {
    let mut dir: Option<&String> = None;
    let mut build: Option<String> = None;
    let mut loader_build: Option<String> = None;
    let mut mc: Option<String> = None;
    let mut dry = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--build" if i + 1 < args.len() => {
                build = Some(args[i + 1].clone());
                i += 2;
            }
            "--loader-build" if i + 1 < args.len() => {
                loader_build = Some(args[i + 1].clone());
                i += 2;
            }
            "--mc" if i + 1 < args.len() => {
                mc = Some(args[i + 1].clone());
                i += 2;
            }
            "--dry-run" => {
                dry = true;
                i += 1;
            }
            otro if !otro.starts_with('-') => {
                dir = Some(&args[i]);
                i += 1;
            }
            _ => i += 1,
        }
    }
    let Some(dir) = dir else {
        eprintln!("uso: forge update <dir> [--build N|latest] [--loader-build N|latest] [--mc V] [--dry-run]");
        eprintln!("  sin --build/--mc: sube a la última build de la misma versión");
        eprintln!("  --build latest: despina la build y vuelve a la última");
        return ExitCode::from(2);
    };
    let (mut cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if puerto_abierto(cfg.port) {
        eprintln!(
            "El server está arriba (puerto {}): apágalo antes (`forge stop {dir}`)",
            cfg.port
        );
        return ExitCode::FAILURE;
    }
    let raiz = resolver(dir);
    let vieja = format!(
        "{} {}",
        cfg.mc_version,
        cfg.build.clone().unwrap_or_default()
    );
    // "latest" despina: vuelve a dejar que el plan elija la última
    if build.as_deref() == Some("latest") {
        cfg.build = None;
        build = None;
    }
    if loader_build.as_deref() == Some("latest") {
        cfg.loader_build = None;
        loader_build = None;
    }
    if let Some(b) = build {
        cfg.build = Some(b);
    }
    if let Some(b) = loader_build {
        cfg.loader_build = Some(b);
    }
    if let Some(v) = mc {
        cfg.mc_version = v;
    }

    let (plan, _avisos) = match forge_api::plan::build_download_plan(&cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("No se pudo resolver el plan nuevo: {e}");
            return ExitCode::FAILURE;
        }
    };
    let engine = plan.entries.iter().find(|e| e.kind == DownloadKind::Engine);
    let Some(engine) = engine else {
        eprintln!("El plan no trae engine (¿config rota?)");
        return ExitCode::FAILURE;
    };
    let nueva = format!(
        "{} {}",
        cfg.mc_version,
        cfg.build.clone().unwrap_or_default()
    );
    if nueva == vieja && raiz.join(&engine.dest).exists() {
        println!("Ya estás en {nueva} (build despinnada o igual): nada que hacer.");
        println!(
            "  para forzar: borra el jar, o `forge update {dir} --build latest` si hay más nuevas."
        );
        return ExitCode::SUCCESS;
    }
    println!("Update: {vieja} → {}", plan.engine_resolved);
    if dry {
        println!(
            "Dry-run: nada se ha escrito. Bajaría {} de {}",
            engine.dest, engine.url
        );
        return ExitCode::SUCCESS;
    }

    // 1) jar nuevo
    let destino = raiz.join(&engine.dest);
    if destino.exists() {
        println!("• {} ya está en disco", engine.dest);
    } else {
        println!("↓ {} …", engine.label);
        if let Err(e) = forge_api::net::download_to(&engine.url, &destino, &Default::default()) {
            eprintln!("Descarga fallida: {e}");
            return ExitCode::FAILURE;
        }
    }
    // 2) scripts/flags/README/openblock.json frescos; el resto es estado del usuario
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);
    let files = generate_project(&cfg, &plan, cores);
    for nombre in [
        "start.sh",
        "start.bat",
        "jvm-args.txt",
        "user_jvm_args.txt",
        "README.md",
        "openblock.json",
    ] {
        if let Some(contenido) = files.get(nombre) {
            if let Err(e) = fs::write(raiz.join(nombre), contenido) {
                eprintln!("escribir {nombre}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    // 3) jar(s) viejo fuera: mismos prefijos, distinto nombre
    for viejo in jars_viejos(&raiz, cfg.engine, &engine.dest) {
        match fs::remove_file(&viejo) {
            Ok(()) => println!(
                "• jar viejo fuera: {}",
                viejo.file_name().unwrap().to_string_lossy()
            ),
            Err(e) => eprintln!("⚠ no pude borrar {}: {e}", viejo.display()),
        }
    }
    // 4) forge/neoforge: el installer rehace libraries/ y run.*
    if plan.installer.is_some() {
        let java_path = fs::read_to_string(raiz.join("runtime").join("java.path"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "java".into());
        println!("== Ejecutando installer (tarda) ==");
        let jar = engine.dest.clone();
        let res = std::process::Command::new(&java_path)
            .args(["-jar", &jar, "--installServer"])
            .current_dir(&raiz)
            .status();
        match res {
            Ok(s) if s.success() => {}
            otra => {
                eprintln!("El installer falló: {otra:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    println!(
        "✓ Actualizado a {}. Mundo, properties, plugins y mods intactos.",
        plan.engine_resolved
    );
    ExitCode::SUCCESS
}

/// jars del engine en la raíz que ya no son el actual (paper-*, purpur-*, …).
fn jars_viejos(raiz: &Path, engine: EngineId, actual: &str) -> Vec<std::path::PathBuf> {
    let prefijos: Vec<String> = match engine {
        EngineId::Paper => vec!["paper-"],
        EngineId::Purpur => vec!["purpur-"],
        EngineId::Folia => vec!["folia-"],
        EngineId::Fabric => vec!["fabric-server-", "fabric-loader-"],
        EngineId::Quilt => vec!["quilt-server-"],
        EngineId::Forge => vec!["forge-installer-", "forge-"],
        EngineId::Neoforge => vec!["neoforge-installer-"],
        EngineId::Vanilla => vec!["server"],
    }
    .into_iter()
    .map(String::from)
    .collect();
    let mut out = Vec::new();
    let Ok(leidos) = fs::read_dir(raiz) else {
        return out;
    };
    for e in leidos.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        if !nombre.ends_with(".jar") || nombre == actual {
            continue;
        }
        if prefijos.iter().any(|p| nombre.starts_with(p.as_str())) {
            out.push(e.path());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jars_viejos_filtra_por_prefijo_y_respeta_el_actual() {
        let dir = std::env::temp_dir().join(format!("ob-upd-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for f in [
            "paper-26.3-175.jar",
            "paper-26.3-170.jar",
            "paper-26.2-100.jar",
            "server.jar",
            "otra-cosa.jar",
            "nota.txt",
        ] {
            fs::write(dir.join(f), b"x").unwrap();
        }
        let viejos = jars_viejos(&dir, EngineId::Paper, "paper-26.3-175.jar");
        let nombres: Vec<String> = viejos
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(nombres.contains(&"paper-26.3-170.jar".to_string()));
        assert!(nombres.contains(&"paper-26.2-100.jar".to_string()));
        assert!(!nombres.contains(&"paper-26.3-175.jar".to_string()));
        assert!(!nombres.contains(&"server.jar".to_string()));
        assert!(!nombres.contains(&"otra-cosa.jar".to_string()));
        fs::remove_dir_all(&dir).ok();
    }
}
