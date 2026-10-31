//! `forge restore <dir>` — restaurar un backup sin miedo.
//!
//! Los backups solo valen si restaurarlos es fácil; si no, nadie los prueba
//! hasta el día del desastre. Esto apaga el server si está arriba, aparta los
//! mundos actuales a `pre-restore-<fecha>/` (por si el backup mismo está mal)
//! y descomprime el backup encima: zip con la librería, tar.gz con el tar del
//! sistema (bsdtar en Windows, que es el que entiende rutas `C:\`).

use crate::players::esperar_puerto_libre;
use crate::rcon::with_rcon;
use crate::status::{cargar_cfg, puerto_abierto};
use crate::{resolver, ExitCode};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn restore_cmd(args: &[String]) -> ExitCode {
    let mut dir: Option<&String> = None;
    let mut fichero: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--file" if i + 1 < args.len() => {
                fichero = Some(args[i + 1].clone());
                i += 2;
            }
            // `--latest` se acepta por escribirlo mentalmente, pero es el default
            "--latest" => {
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
        eprintln!("uso: forge restore <dir> [--file backup.zip|tar.gz]");
        eprintln!("  sin --file: coge el más reciente de backups/ (--latest es lo mismo)");
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

    // 0) apagar si está arriba (los ficheros dentro del zip son del server parado)
    if puerto_abierto(cfg.port) {
        if !cfg.rcon.enabled {
            eprintln!("El server está arriba y sin RCON: apágalo a mano y repite");
            return ExitCode::FAILURE;
        }
        println!("Apagando el server (stop por RCON)…");
        if let Err(e) = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
            r.command("stop")
        }) {
            eprintln!("No se pudo apagar: {e}");
            return ExitCode::FAILURE;
        }
        if !esperar_puerto_libre(cfg.port, 90) {
            eprintln!("El server no se apagó en 90 s (¿guardando chunks?): inténtalo otra vez");
            return ExitCode::FAILURE;
        }
    }

    // 1) elegir backup
    let backup = match fichero {
        Some(f) => resolver(&f),
        None => match backup_mas_reciente(&raiz) {
            Some(b) => b,
            None => {
                eprintln!(
                    "No hay backups en {}: haz uno con `forge backup {dir}`",
                    raiz.join("backups").display()
                );
                return ExitCode::FAILURE;
            }
        },
    };
    println!("Restaurando {} …", backup.display());

    // 2) mundos actuales apartados, no borrados: si el backup está mal, siguen ahí
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let seguridad = raiz.join(format!("pre-restore-{ts}"));
    let mut apartados = 0;
    for mundo in ["world", "world_nether", "world_the_end"] {
        let p = raiz.join(mundo);
        if p.exists() {
            fs::create_dir_all(&seguridad).ok();
            if let Err(e) = fs::rename(&p, seguridad.join(mundo)) {
                eprintln!("apartar {}: {e}", p.display());
                return ExitCode::FAILURE;
            }
            apartados += 1;
        }
    }
    if apartados > 0 {
        println!(
            "• {apartados} mundo(s) actuales apartados en {}",
            seguridad.display()
        );
    }

    // 3) descomprimir encima
    let nombre = backup
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let res = if nombre.ends_with(".zip") {
        descomprimir_zip(&backup, &raiz)
    } else if nombre.ends_with(".tar.gz") || nombre.ends_with(".tgz") {
        descomprimir_tar(&backup, &raiz)
    } else {
        Err(format!("no sé restaurar «{nombre}»: solo .zip y .tar.gz"))
    };
    if let Err(e) = res {
        eprintln!("Restauración fallida: {e}");
        if apartados > 0 {
            eprintln!("Los mundos anteriores siguen en {}", seguridad.display());
        }
        return ExitCode::FAILURE;
    }
    println!("✓ Restaurado. Los mundos que había antes están en {} (bórralos cuando compruebes que todo va).", seguridad.display());
    println!("  Arranca con start.sh / start.bat y `forge status {dir}` para comprobar.");
    ExitCode::SUCCESS
}

/// el backup más nuevo de backups/ (backup-*), por fecha de modificación
fn backup_mas_reciente(raiz: &Path) -> Option<PathBuf> {
    let backups = raiz.join("backups");
    let mut mejor: Option<(SystemTime, PathBuf)> = None;
    for e in fs::read_dir(&backups).ok()?.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        if !nombre.starts_with("backup-") {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let Ok(t) = meta.modified() else { continue };
        if mejor.as_ref().is_none_or(|(mt, _)| t > *mt) {
            mejor = Some((t, e.path()));
        }
    }
    mejor.map(|(_, p)| p)
}

fn descomprimir_zip(zip_path: &Path, raiz: &Path) -> Result<(), String> {
    let fichero =
        fs::File::open(zip_path).map_err(|e| format!("abrir {}: {e}", zip_path.display()))?;
    let mut zip = zip::ZipArchive::new(fichero).map_err(|e| format!("zip ilegible: {e}"))?;
    zip.extract(raiz).map_err(|e| format!("extraer: {e}"))?;
    Ok(())
}

fn descomprimir_tar(tar_path: &Path, raiz: &Path) -> Result<(), String> {
    // bsdtar (System32) en Windows entiende rutas C:\ y lee tar.gz; GNU tar en Linux
    let tar = if cfg!(windows) {
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        PathBuf::from(sys).join("System32").join("tar.exe")
    } else {
        PathBuf::from("tar")
    };
    let out = Command::new(&tar)
        .arg("-xzf")
        .arg(tar_path)
        .arg("-C")
        .arg(raiz)
        .output()
        .map_err(|e| format!("lanzar tar: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "tar: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_de_prueba(dest: &Path) {
        let f = fs::File::create(dest).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        w.start_file("world/level.dat", opts).unwrap();
        w.write_all(b"nivel").unwrap();
        w.start_file("server.properties", opts).unwrap();
        w.write_all(b"server-port=25565").unwrap();
        w.finish().unwrap();
    }

    #[test]
    fn zip_se_restaura_y_aparta_los_mundos() {
        let dir = std::env::temp_dir().join(format!("ob-rst-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        fs::create_dir_all(dir.join("backups")).unwrap();
        fs::write(dir.join("world").join("level.dat"), b"nuevo-que-esta-mal").unwrap();
        let zip_path = dir.join("backups").join("backup-20260101-000000.zip");
        zip_de_prueba(&zip_path);

        // mundo actual existe → se aparta; luego el zip deja el suyo
        let ts = 12345u64;
        let seguridad = dir.join(format!("pre-restore-{ts}"));
        fs::create_dir_all(&seguridad).unwrap();
        fs::rename(dir.join("world"), seguridad.join("world")).unwrap();
        descomprimir_zip(&zip_path, &dir).unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("world").join("level.dat")).unwrap(),
            "nivel"
        );
        assert_eq!(
            fs::read_to_string(seguridad.join("world").join("level.dat")).unwrap(),
            "nuevo-que-esta-mal"
        );
        assert!(dir.join("server.properties").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_mas_reciente_gana() {
        let dir = std::env::temp_dir().join(format!("ob-rst2-{}", std::process::id()));
        fs::create_dir_all(dir.join("backups")).unwrap();
        fs::write(dir.join("backups").join("backup-viejo.zip"), b"1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(dir.join("backups").join("backup-nuevo.zip"), b"2").unwrap();
        fs::write(dir.join("backups").join("otro.txt"), b"3").unwrap();
        let elegido = backup_mas_reciente(&dir).unwrap();
        assert_eq!(elegido.file_name().unwrap(), "backup-nuevo.zip");
        fs::remove_dir_all(&dir).ok();
    }
}
