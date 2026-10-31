//! `forge export <dir>` — el server instalado, convertido en zip portable.
//!
//! El `--zip` del generate empaqueta lo que OpenBlock genera; esto empaqueta
//! lo que hay **instalado** —plugins incluidos— para moverlo a otra máquina o
//! guardarlo como plantilla. Se excluye lo que no viaja: mundos, logs,
//! backups, runtime de Java, libraries del installer y restos. Con `--world`
//! se incluyen los mundos también (para mudanza completa, no plantilla).

use crate::{resolver, ExitCode};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn export_cmd(args: &[String]) -> ExitCode {
    let mut dir: Option<&String> = None;
    let mut out: Option<String> = None;
    let mut con_mundos = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" | "--zip" if i + 1 < args.len() => {
                out = Some(args[i + 1].clone());
                i += 2;
            }
            "--world" => {
                con_mundos = true;
                i += 1;
            }
            otro if !otro.starts_with('-') && dir.is_none() => {
                dir = Some(&args[i]);
                i += 1;
            }
            _ => i += 1,
        }
    }
    let Some(dir) = dir else {
        eprintln!("uso: forge export <dir> [--out server.zip] [--world]");
        eprintln!("  --world incluye los mundos (mudanza completa; sin él, plantilla)");
        return ExitCode::from(2);
    };
    let raiz = resolver(dir);
    if !raiz.join("openblock.json").exists() {
        eprintln!(
            "No hay openblock.json en {}: no es un server de OpenBlock",
            raiz.display()
        );
        return ExitCode::FAILURE;
    }
    let cfg: forge_core::ServerConfig = match fs::read_to_string(raiz.join("openblock.json")) {
        Ok(t) => match serde_json::from_str(&t) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("openblock.json inválido: {e}");
                return ExitCode::FAILURE;
            }
        },
        Err(e) => {
            eprintln!("leer openblock.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let salida = out.unwrap_or_else(|| format!("{}-portable.zip", cfg.name));
    let ruta = resolver(&salida);

    let fichero = File::create(&ruta).map_err(|e| format!("crear {}: {e}", ruta.display()));
    let fichero = match fichero {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut zip = zip::ZipWriter::new(fichero);
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut n = 0u64;
    let mut bytes = 0u64;
    if let Err(e) = empaquetar(
        &raiz, &raiz, &mut zip, &opts, con_mundos, &mut n, &mut bytes,
    ) {
        eprintln!("empaquetar: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = zip.finish() {
        eprintln!("cerrar zip: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "✓ {} ficheros ({:.1} MB) en {}",
        n,
        bytes as f64 / 1048576.0,
        ruta.display()
    );
    println!("  en la otra máquina: unzip, ./install.sh (o install.ps1), y start");
    ExitCode::SUCCESS
}

/// lo que nunca viaja en un export
fn excluido(nombre: &str, con_mundos: bool) -> bool {
    // privados de sesión (usercache, baneos) fuera siempre: no son de la plantilla
    let siempre = matches!(
        nombre,
        "logs"
            | "backups"
            | "runtime"
            | "libraries"
            | "cache"
            | "node_modules"
            | "plugins_disabled"
            | "usercache.json"
            | "banned-players.json"
            | "banned-ips.json"
    ) || nombre.starts_with("pre-restore-")
        || nombre.ends_with(".log");
    if con_mundos {
        siempre
    } else {
        siempre || matches!(nombre, "world" | "world_nether" | "world_the_end")
    }
}

fn empaquetar(
    raiz: &Path,
    actual: &Path,
    zip: &mut zip::ZipWriter<File>,
    opts: &zip::write::FileOptions<'_, ()>,
    con_mundos: bool,
    n: &mut u64,
    bytes: &mut u64,
) -> Result<(), String> {
    for e in fs::read_dir(actual).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let nombre = e.file_name().to_string_lossy().to_string();
        if excluido(&nombre, con_mundos) {
            continue;
        }
        let ruta = e.path();
        let rel = ruta
            .strip_prefix(raiz)
            .unwrap_or(&ruta)
            .to_string_lossy()
            .replace('\\', "/");
        if e.metadata().map(|m| m.is_dir()).unwrap_or(false) {
            // directorio: recursión (los zip no necesitan entradas de directorio)
            empaquetar(raiz, &ruta, zip, opts, con_mundos, n, bytes)?;
        } else {
            let datos = fs::read(&ruta).map_err(|e| e.to_string())?;
            zip.start_file(rel.clone(), *opts)
                .map_err(|e| e.to_string())?;
            zip.write_all(&datos).map_err(|e| e.to_string())?;
            *n += 1;
            *bytes += datos.len() as u64;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::excluido;

    #[test]
    fn lo_que_no_viaja() {
        assert!(excluido("logs", false));
        assert!(excluido("backups", false));
        assert!(excluido("runtime", false));
        assert!(excluido("pre-restore-123", false));
        assert!(excluido("boot.log", false));
        assert!(excluido("world", false));
        assert!(excluido("banned-players.json", false));
        // con --world los mundos sí van; lo privado de sesión también
        assert!(!excluido("world", true));
        assert!(!excluido("plugins", false));
        assert!(!excluido("server.properties", false));
        assert!(!excluido("openblock.json", false));
        assert!(excluido("usercache.json", true));
    }
}
