//! Gestión de Java, portada de `src/java.ts`.
//!
//! Usa el JDK del sistema si cubre la versión que pide MC; si no, baja Temurin
//! (Adoptium) a `<serverDir>/runtime/jdkN`, y queda encapsulado: el server nunca
//! toca el Java del sistema.
//!
//! La extracción se hace con el `tar` del sistema, **igual que el TS**: en Windows
//! el `tar.exe` de System32 (bsdtar) extrae el `.zip` de Adoptium, y en Linux el
//! `tar` de siempre. No hace falta traer un `flate2` ni un `zip` para esto, y de
//! paso se reproduce el detalle del TS que usa `-xf` en Windows y `-xzf` en Linux.

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Timeout de `java -version`, el mismo que el TS (`execFile` con 15000 ms).
const JAVA_TIMEOUT: Duration = Duration::from_secs(15);
/// Timeout de la extracción del JDK (`tar` con 300000 ms en el TS).
const TAR_TIMEOUT: Duration = Duration::from_secs(300);

/// Un Java utilizable (`JavaRuntime`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRuntime {
    pub path: String,
    pub major: u32,
    /// `true` si lo bajó OpenBlock a `runtime/`, `false` si es del sistema.
    pub managed: bool,
}

/// La versión mayor de un `java` (`javaMajorOf`), o `None` si no se puede saber.
///
/// `execFile` con `-version`: gana el `stderr`, y si está vacío el `stdout`. Java 8
/// se anuncia como `1.8.0_402`, así que el `1` inicial significa «mira el segundo
/// número».
pub fn java_major_of(java_path: &str) -> Option<u32> {
    let salida = ejecutar(java_path, &["-version"], JAVA_TIMEOUT)?;
    let texto = if salida.1.trim().is_empty() {
        salida.0
    } else {
        salida.1
    };
    let re = regex_lite(r#"version "(\d+)(?:\.(\d+))?"#)?;
    let caps = re.captures(&texto)?;
    let mayor: u32 = caps.get(1)?.as_str().parse().ok()?;
    if mayor == 1 {
        caps.get(2)?.as_str().parse().ok()
    } else {
        Some(mayor)
    }
}

/// Un Java del sistema que cubra esa versión (`findSystemJava`).
///
/// **No se ordena la lista de directorios a propósito**: `readdir` no garantiza
/// orden en ninguno de los dos lados, pero los dos preguntan al mismo sistema de
/// ficheros, así que ordenar aquí sería cambiar en silencio cuál se elige. Y eso
/// importa: en Windows conviven `jdk-21…` y `jre-21…`, los dos reportan 21, y el
/// que gane acaba dentro de `runtime/java.path`.
pub fn find_system_java(major: u32) -> Option<JavaRuntime> {
    let mut candidatos: Vec<String> = Vec::new();
    if cfg!(windows) {
        for base in [
            "C:\\Program Files\\Java",
            "C:\\Program Files\\Eclipse Adoptium",
            "C:\\Program Files\\Microsoft",
            "C:\\Program Files\\Zulu",
        ] {
            if let Ok(entradas) = std::fs::read_dir(base) {
                for e in entradas.flatten() {
                    candidatos.push(
                        Path::new(base)
                            .join(e.file_name())
                            .join("bin")
                            .join("java.exe")
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    } else if let Ok(entradas) = std::fs::read_dir("/usr/lib/jvm") {
        for e in entradas.flatten() {
            candidatos.push(
                Path::new("/usr/lib/jvm")
                    .join(e.file_name())
                    .join("bin")
                    .join("java")
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    // el PATH siempre es el último recurso
    candidatos.push(if cfg!(windows) { "java.exe" } else { "java" }.to_string());

    for c in candidatos {
        if java_major_of(&c) == Some(major) {
            return Some(JavaRuntime {
                path: c,
                major,
                managed: false,
            });
        }
    }
    None
}

/// Un Java de esa versión, del sistema o bajado (`ensureJava`).
pub fn ensure_java(major: u32, server_dir: &Path) -> Result<JavaRuntime, String> {
    if let Some(sistema) = find_system_java(major) {
        return Ok(sistema);
    }
    download_java(major, server_dir)
}

/// Baja un Temurin de Adoptium y lo deja en `runtime/jdkN` (`downloadJava`).
pub fn download_java(major: u32, server_dir: &Path) -> Result<JavaRuntime, String> {
    let os_name = if cfg!(windows) { "windows" } else { "linux" };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os={os_name}&vendor=eclipse"
    );
    let assets: Value = forge_api::net::get_json(&url).map_err(|e| format!("Adoptium: {e}"))?;
    let (link, nombre) = paquete_adoptium(&assets)
        .ok_or_else(|| format!("Adoptium sin paquete JDK {major} para {os_name}/{arch}"))?;

    let runtime_dir = server_dir.join("runtime");
    let extract_dir = runtime_dir.join(format!("jdk{major}"));
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    let archivo = runtime_dir.join(&nombre);
    forge_api::net::download_to(
        &link,
        &archivo,
        &forge_api::net::OpcionesDescarga::default(),
    )
    .map_err(|e| format!("{e}"))?;

    // el `tar` de Windows (bsdtar, System32) extrae zip; el de Git Bash (GNU)
    // interpreta "C:" como host remoto, así que se llama al de System32
    let tar = if cfg!(windows) {
        Path::new(&std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string()))
            .join("System32")
            .join("tar.exe")
            .to_string_lossy()
            .to_string()
    } else {
        "tar".to_string()
    };
    let mut cmd = Command::new(&tar);
    cmd.arg(if cfg!(windows) { "-xf" } else { "-xzf" })
        .arg(&archivo)
        .arg("-C")
        .arg(&extract_dir);
    let salida =
        ejecutar_cmd(&mut cmd, TAR_TIMEOUT).ok_or_else(|| format!("no se pudo ejecutar {tar}"))?;
    if !salida.2 {
        return Err(format!(
            "tar falló extrayendo el JDK {major}: {}",
            salida.1.trim()
        ));
    }

    // Adoptium mete todo en `jdk-21.0.x+y`: si hay una sola entrada, esa es la raíz
    let entradas: Vec<String> = std::fs::read_dir(&extract_dir)
        .map(|e| {
            e.flatten()
                .map(|x| x.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    let root = match raiz_extraida(&entradas) {
        Some(inner) => extract_dir.join(inner),
        None => extract_dir,
    };

    let java_bin = if cfg!(windows) {
        root.join("bin").join("java.exe")
    } else {
        root.join("bin").join("java")
    };
    let java_bin = java_bin.to_string_lossy().to_string();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for f in ["java", "keytool"] {
            let p = root.join("bin").join(f);
            if let Ok(meta) = std::fs::metadata(&p) {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                let _ = std::fs::set_permissions(&p, perm);
            }
        }
    }

    let comprobado = java_major_of(&java_bin);
    if comprobado != Some(major) {
        return Err(format!(
            "JDK extraído no reporta versión {major}: {java_bin} -> {}",
            match comprobado {
                Some(m) => m.to_string(),
                None => "null".to_string(),
            }
        ));
    }
    Ok(JavaRuntime {
        path: java_bin,
        major,
        managed: true,
    })
}

/// El enlace y el nombre del paquete de la respuesta de Adoptium
/// (`assets[0].binary.package`).
///
/// Es puro a propósito: la respuesta de Adoptium cambia y el resto del camino
/// —bajar 180 MB y descomprimirlos— no se puede probar en cada `cargo test`.
pub fn paquete_adoptium(assets: &Value) -> Option<(String, String)> {
    let pkg = assets.get(0)?.get("binary")?.get("package")?;
    let link = pkg.get("link")?.as_str()?.to_string();
    let nombre = pkg.get("name")?.as_str()?.to_string();
    Some((link, nombre))
}

/// La raíz real dentro del directorio de extracción.
///
/// Adoptium mete todo en una carpeta (`jdk-21.0.11.10-hotspot`), así que si el
/// directorio tiene una sola entrada, esa es la raíz. Con cero o varias, el
/// directorio tal cual — que es lo que hace el TS.
pub fn raiz_extraida(entradas: &[String]) -> Option<String> {
    if entradas.len() == 1 {
        Some(entradas[0].clone())
    } else {
        None
    }
}

// ---------- procesos ----------

fn regex_lite(patron: &str) -> Option<regex::Regex> {
    regex::Regex::new(patron).ok()
}

/// Ejecuta y devuelve `(stdout, stderr)`.
fn ejecutar(programa: &str, args: &[&str], timeout: Duration) -> Option<(String, String)> {
    let mut cmd = Command::new(programa);
    cmd.args(args);
    let salida = ejecutar_cmd(&mut cmd, timeout)?;
    Some((salida.0, salida.1))
}

/// Ejecuta y devuelve `(stdout, stderr, fue_bien)`.
///
/// `Command` no trae timeout: se sondea `try_wait` y se mata al pasarse. Vale para
/// `java -version` y para `tar`, que escriben poco por los pipes.
fn ejecutar_cmd(cmd: &mut Command, timeout: Duration) -> Option<(String, String, bool)> {
    let mut hijo = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let limite = Instant::now() + timeout;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= limite {
                    let _ = hijo.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
    let salida = hijo.wait_with_output().ok()?;
    Some((
        String::from_utf8_lossy(&salida.stdout).to_string(),
        String::from_utf8_lossy(&salida.stderr).to_string(),
        salida.status.success(),
    ))
}

/// Una ruta de Java normalizada para escribirla en `runtime/java.path`.
pub fn ruta_para_archivo(rt: &JavaRuntime) -> String {
    format!("{}\n", rt.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_raiz_es_la_unica_entrada_si_hay_una() {
        assert_eq!(
            raiz_extraida(&["jdk-21.0.11.10-hotspot".to_string()]),
            Some("jdk-21.0.11.10-hotspot".to_string())
        );
        assert_eq!(raiz_extraida(&[]), None);
        assert_eq!(
            raiz_extraida(&["a".to_string(), "b".to_string()]),
            None,
            "con varias entradas el TS se queda con el directorio tal cual"
        );
    }

    #[test]
    fn el_paquete_de_adoptium_se_lee_donde_esta() {
        let g: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/fixtures/adoptium-21.json"
        )))
        .expect("fixture de Adoptium");
        let (link, nombre) = paquete_adoptium(&g).expect("hay paquete");
        assert!(link.ends_with(".zip"), "{link}");
        assert!(
            nombre.starts_with("OpenJDK21U-jdk_x64_windows_hotspot_"),
            "{nombre}"
        );
        // y una respuesta sin paquete no revienta: devuelve None
        assert!(paquete_adoptium(&serde_json::json!([])).is_none());
        assert!(paquete_adoptium(&serde_json::json!([{"binary": {}}])).is_none());
    }

    #[test]
    fn encuentra_el_java_del_path_para_su_propia_version() {
        // autoconsistente: se pregunta por la versión que reporta el `java` del PATH
        let Some(mayor) = java_major_of(if cfg!(windows) { "java.exe" } else { "java" }) else {
            return; // sin java en el PATH no hay nada que comprobar
        };
        let rt = find_system_java(mayor).expect("el del PATH como último recurso");
        assert_eq!(rt.major, mayor);
        assert!(!rt.managed, "el del sistema no lo gestiona OpenBlock");
        assert_eq!(java_major_of(&rt.path), Some(mayor));
        assert!(rt.path.ends_with("java") || rt.path.ends_with("java.exe"));
    }

    #[test]
    fn una_version_imposible_no_encuentra_nada() {
        // 99 no existe; no se baja nada, solo se busca
        assert!(find_system_java(99).is_none());
    }
}
