//! `forge logs <dir>` — el log del server sin abrir el archivo a mano.
//!
//! Tail de `logs/latest.log` con `-f` (sigue el fichero, como tail -f) y
//! filtros: `--errors` deja pasar solo lo que parece grave (WARN/ERROR y
//! stacktraces) y `--player Ana` solo las líneas donde aparece Ana. Los
//! filtros se pueden combinar.

use crate::{resolver, ExitCode};
use std::fs;
use std::io::{Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

pub fn logs_cmd(args: &[String]) -> ExitCode {
    let mut dir: Option<&String> = None;
    let mut seguir = false;
    let mut solo_errores = false;
    let mut jugador: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-f" | "--follow" => {
                seguir = true;
                i += 1;
            }
            "--errors" => {
                solo_errores = true;
                i += 1;
            }
            "--player" if i + 1 < args.len() => {
                jugador = Some(args[i + 1].clone());
                i += 2;
            }
            otro if !otro.starts_with('-') && dir.is_none() => {
                dir = Some(&args[i]);
                i += 1;
            }
            _ => i += 1,
        }
    }
    let Some(dir) = dir else {
        eprintln!("uso: forge logs <dir> [-f] [--errors] [--player <nombre>]");
        return ExitCode::from(2);
    };
    let log = resolver(dir).join("logs").join("latest.log");
    if !log.exists() {
        eprintln!(
            "No existe {}: arranca el server una vez primero",
            log.display()
        );
        return ExitCode::FAILURE;
    }

    let pasa = |linea: &str| -> bool {
        if solo_errores && !es_grave(linea) {
            return false;
        }
        if let Some(j) = &jugador {
            return linea.to_lowercase().contains(&j.to_lowercase());
        }
        true
    };

    // lo que ya hay + seguir creciendo si -f
    let mut fh = fs::File::open(&log).expect("acaba de existir");
    let mut pos = if seguir {
        // con -f empezamos por el final: lo histórico ya pasó
        fh.seek(SeekFrom::End(0)).unwrap_or(0)
    } else {
        // sin -f, el final vivo: hasta 200 líneas atrás
        let fin = fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
        let inicio = fin.saturating_sub(16 * 1024);
        fh.seek(SeekFrom::Start(inicio)).unwrap_or(0)
    };
    loop {
        let mut buf = vec![0u8; 8192];
        use std::io::Read;
        match fh.read(&mut buf) {
            Ok(0) => {
                if !seguir {
                    break;
                }
                // el log rota en cada arranque: si el inode cambió, reabrir
                if rotado(&log, &pos) {
                    fh = fs::File::open(&log).unwrap();
                    fh.seek(SeekFrom::Start(0)).ok();
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Ok(n) => {
                pos += n as u64;
                for linea in String::from_utf8_lossy(&buf[..n]).split('\n') {
                    let l = linea.trim_end_matches('\r');
                    if !l.is_empty() && pasa(l) {
                        println!("{l}");
                    }
                }
            }
            Err(e) => {
                eprintln!("leer log: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// ¿la línea es WARN/ERROR o parte de un stacktrace?
fn es_grave(linea: &str) -> bool {
    linea.contains("WARN")
        || linea.contains("ERROR")
        || linea.contains("Exception")
        || linea.contains("\tat ")
}

/// ¿el latest.log fue reemplazado (arranque nuevo)? tamaño menor que donde íbamos
fn rotado(log: &Path, pos: &u64) -> bool {
    fs::metadata(log).map(|m| m.len() < *pos).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::es_grave;

    #[test]
    fn el_filtro_de_graves_caza_lo_que_importa() {
        assert!(es_grave("[12:00 WARN]: algo"));
        assert!(es_grave("[12:00 ERROR]: algo"));
        assert!(es_grave("java.lang.NulledException: boom"));
        assert!(es_grave("\tat com.example.Foo(Foo.java:1)"));
        assert!(!es_grave("[12:00 INFO]: Done (1s)!"));
        assert!(!es_grave("Steve joined the game"));
    }
}
