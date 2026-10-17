//! `apply`: convierte config + plan en un server **real** en disco, portado de
//! `src/apply.ts`.
//!
//! Deja `start.sh`/`start.bat` listos, Java encapsulado en `runtime/`, los
//! plugins/mods descargados y (si toca) el installer de Forge/NeoForge ejecutado.
//!
//! Lo que **no** está portado todavía: la pregeneración por RCON (`pregenerate`),
//! `stopServer`, `runBackup` y `extractCfOverrides`. Los tres primeros son de los
//! subcomandos `pregen`/`stop`/`backup` del modo avanzado, que tampoco están.

use forge_api::plan::build_download_plan;
use forge_core::generators::{generate_project, validate};
use forge_core::plan::DownloadPlan;
use forge_core::types::ServerConfig;
use std::path::Path;

/// El `FileMap` que produce el núcleo: ruta relativa → contenido.
type FileMap = std::collections::BTreeMap<String, String>;

/// Opciones de `applyConfig` (`ApplyOptions`).
#[derive(Debug, Clone, Default)]
pub struct OpcionesApply {
    pub cores: u32,
    pub dry_run: bool,
    /// fuerza re-descarga aunque el fichero exista
    pub force: bool,
}

/// Escribe el `FileMap` en un directorio (`writeProject`).
///
/// LF en todo menos en los `.bat`, que van con CRLF: el `FileMap` del núcleo está
/// en LF —lo comprueba el golden— y el salto de Windows se añade aquí, al
/// materializarlo. Es el mismo reparto que en el TS.
pub fn write_project(dir: &Path, files: &FileMap) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (nombre, contenido) in files {
        let destino = dir.join(nombre);
        if let Some(padre) = destino.parent() {
            std::fs::create_dir_all(padre)?;
        }
        let eol = if nombre.ends_with(".bat") {
            "\r\n"
        } else {
            "\n"
        };
        std::fs::write(&destino, contenido.replace('\n', eol))?;
    }
    Ok(())
}

/// El nombre de fichero que sugiere una URL (`basename(new URL(u).pathname)`).
///
/// Sin query ni fragmento: una URL de CDN con `?v=2` acabaría en un fichero
/// llamado `x.jar?v=2`. El `URL` del TS tampoco lo incluye.
pub fn basename_de_url(url: &str) -> String {
    let sin_esquema = url.split("://").nth(1).unwrap_or(url);
    let ruta = match sin_esquema.split_once('/') {
        Some((_, r)) => r,
        None => "",
    };
    let ruta = ruta.split(['?', '#']).next().unwrap_or("");
    ruta.rsplit('/').next().unwrap_or("").to_string()
}

/// Aplica config y plan al disco (`applyConfig`).
pub fn apply_config(
    cfg: &mut ServerConfig,
    opts: &OpcionesApply,
    log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let errores = validate(cfg);
    if !errores.is_empty() {
        return Err(format!("Config inválida:\n  - {}", errores.join("\n  - ")));
    }
    let dir = crate::resolver(&cfg.server_dir);
    let (plan, avisos) = build_download_plan(cfg).map_err(|e| e.to_string())?;

    log(format!("== Plan == {}", plan.engine_resolved));
    for e in &plan.entries {
        log(format!("  [{}] {}", e.kind.as_str(), e.label));
    }
    if plan.unresolved_cf == Some(-1) {
        log("  ⚠ Modpack CurseForge: instala el modpack con el launcher (o define OPENBLOCK_CF_KEY) y copia mods/ + overrides al server.".to_string());
    } else if let Some(n) = plan.unresolved_cf {
        if n > 0 {
            log(format!(
                "  ⚠ {n} items del catálogo sin build para {}: se saltan.",
                cfg.mc_version
            ));
        }
    }
    for a in &avisos {
        log(a.0.clone());
    }

    if opts.dry_run {
        log("Dry-run: nada se ha escrito.".to_string());
        return Ok(());
    }

    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let files = generate_project(cfg, &plan, opts.cores);
    write_project(&dir, &files).map_err(|e| format!("{}: {e}", dir.display()))?;
    log(format!("✓ Proyecto escrito en {}", dir.display()));

    // 1) el Java correcto para esta versión de MC, encapsulado en runtime/
    let java =
        crate::java::ensure_java(forge_core::catalog::java_major_for(&cfg.mc_version), &dir)?;
    if java.managed {
        log(format!(
            "✓ JDK {} (Temurin) instalado en runtime/",
            java.major
        ));
    } else {
        log(format!(
            "✓ Java del sistema usado: {} (v{})",
            java.path, java.major
        ));
    }
    let runtime = dir.join("runtime");
    std::fs::create_dir_all(&runtime).map_err(|e| e.to_string())?;
    std::fs::write(
        runtime.join("java.path"),
        crate::java::ruta_para_archivo(&java),
    )
    .map_err(|e| e.to_string())?;

    // 2) descargas: engine, plugins/mods, modpack y custom
    descargar_entradas(&dir, &plan, opts.force, log)?;

    // 2b) ficheros locales del usuario (custom items con path)
    for lc in &plan.local_copies {
        let src = Path::new(&lc.src);
        if !src.exists() {
            log(format!("⚠ Custom local no existe: {}", lc.src));
            continue;
        }
        let destino = dir.join(&lc.dest);
        if let Some(padre) = destino.parent() {
            std::fs::create_dir_all(padre).map_err(|e| e.to_string())?;
        }
        std::fs::copy(src, &destino).map_err(|e| format!("{}: {e}", destino.display()))?;
        log(format!(
            "✓ Copiado {} a {}",
            src.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default(),
            lc.dest
        ));
    }

    // 3) installer de forge/neoforge (descarga las librerías del loader)
    if let Some(inst) = &plan.installer {
        let libs = dir.join("libraries");
        if libs.exists() && !opts.force {
            log("• installer ya ejecutado (libraries/ presente)".to_string());
        } else {
            log(format!(
                "== Ejecutando installer de {} (tarda) ==",
                inst.engine
            ));
            run_java(
                &java.path,
                &["-jar", &inst.jar, "--installServer"],
                &dir,
                log,
            )?;
        }
    }

    log(format!(
        "✓ Server instalado. Arranca con: {} (en {})",
        if cfg!(windows) {
            "start.bat"
        } else {
            "./start.sh"
        },
        dir.display()
    ));
    Ok(())
}

/// La descarga de todas las entradas del plan, saltando lo que ya está.
///
/// El `existsSync(dest) → stat(dest).size > 0` del TS es lo que hace que un
/// `install` repetido sea barato: un fichero de 0 bytes (una descarga cortada) sí
/// se vuelve a bajar.
fn descargar_entradas(
    dir: &Path,
    plan: &DownloadPlan,
    force: bool,
    log: &mut dyn FnMut(String),
) -> Result<(), String> {
    for e in &plan.entries {
        let rel = if e.dest.is_empty() {
            basename_de_url(&e.url)
        } else {
            e.dest.clone()
        };
        let destino = dir.join(&rel);
        if destino.exists() && !force {
            let tam = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
            if tam > 0 {
                log(format!("• {rel} ya existe, se salta"));
                continue;
            }
        }
        if let Some(padre) = destino.parent() {
            std::fs::create_dir_all(padre).map_err(|e| e.to_string())?;
        }
        log(format!("↓ {} ...", e.label));
        forge_api::net::download_to(
            &e.url,
            &destino,
            &forge_api::net::OpcionesDescarga {
                sha256: e.sha256.as_deref(),
                sha512: e.sha512.as_deref(),
            },
        )
        .map_err(|err| format!("{err}"))?;
    }
    Ok(())
}

/// Ejecuta el installer y espera a que termine (`runJava`).
fn run_java(
    java_path: &str,
    args: &[&str],
    cwd: &Path,
    log: &mut dyn FnMut(String),
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut hijo = Command::new(java_path)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar el installer: {e}"))?;

    // los dos pipes, en hilos, para que el installer no se quede bloqueado.
    // Van en un `Box<dyn Read>` porque `ChildStdout` y `ChildStderr` son tipos
    // distintos y el hilo tiene que poder leer cualquiera de los dos.
    use std::io::Read;
    let mut lectores: Vec<Box<dyn Read + Send>> = Vec::new();
    if let Some(out) = hijo.stdout.take() {
        lectores.push(Box::new(out));
    }
    if let Some(err) = hijo.stderr.take() {
        lectores.push(Box::new(err));
    }
    let mut lineas: Vec<String> = Vec::new();
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for lector in lectores {
            handles.push(s.spawn(move || {
                let mut v = Vec::new();
                for l in BufReader::new(lector).lines().map_while(Result::ok) {
                    if !l.trim().is_empty() {
                        v.push(l.trim().to_string());
                    }
                }
                v
            }));
        }
        for h in handles {
            if let Ok(v) = h.join() {
                lineas.extend(v);
            }
        }
    });
    for l in lineas {
        log(format!("  {l}"));
    }

    let estado = hijo.wait().map_err(|e| e.to_string())?;
    if estado.success() {
        Ok(())
    } else {
        Err(format!(
            "Installer salió con código {}",
            estado.code().unwrap_or(-1)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_de_fichero_de_una_url() {
        assert_eq!(
            basename_de_url("https://cdn.modrinth.com/data/x/Chunky-1.3.jar"),
            "Chunky-1.3.jar"
        );
        assert_eq!(basename_de_url("https://x/y.jar?v=2"), "y.jar");
        assert_eq!(basename_de_url("https://x/y.jar#frag"), "y.jar");
        assert_eq!(basename_de_url("https://x/"), "");
    }

    #[test]
    fn el_proyecto_se_escribe_con_crlf_en_los_bat() {
        let dir = std::env::temp_dir().join(format!("openblock-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut files: FileMap = std::collections::BTreeMap::new();
        files.insert("start.sh".to_string(), "linea1\nlinea2\n".to_string());
        files.insert("start.bat".to_string(), "linea1\nlinea2\n".to_string());
        files.insert("plugins/x.txt".to_string(), "sub\n".to_string());
        write_project(&dir, &files).unwrap();
        assert_eq!(
            std::fs::read(dir.join("start.sh")).unwrap(),
            b"linea1\nlinea2\n".to_vec()
        );
        assert_eq!(
            std::fs::read(dir.join("start.bat")).unwrap(),
            b"linea1\r\nlinea2\r\n".to_vec()
        );
        assert!(
            dir.join("plugins/x.txt").exists(),
            "los subdirectorios se crean"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_config_invalida_no_escribe_nada() {
        let mut cfg: ServerConfig = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/fixtures/parity-config.json"
        )))
        .unwrap();
        cfg.port = 80; // fuera del rango permitido
        cfg.server_dir = std::env::temp_dir()
            .join(format!("openblock-invalida-{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        let err = apply_config(&mut cfg, &OpcionesApply::default(), &mut |_| {})
            .expect_err("puerto 80 no vale");
        assert!(
            err.starts_with("Config inválida:\n  - Puerto fuera de rango"),
            "{err}"
        );
        assert!(
            !std::path::Path::new(&cfg.server_dir).exists(),
            "no debe escribir nada"
        );
    }
}
