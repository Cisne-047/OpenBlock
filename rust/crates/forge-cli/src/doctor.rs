//! El doctor en Rust: la parte que sí toca el disco y el sistema.
//!
//! El núcleo (`forge_core::doctor`) tiene la aritmética, las reglas de log y las
//! reglas estáticas, puro todo. Aquí se le dan las dos cosas que le faltan:
//!
//! - `VistaDisco`, que es `VistaServidor` sobre un directorio de verdad.
//! - `HechosReales`, que responde a las tres preguntas que dependen de la máquina:
//!   la versión de Java instalada, si el puerto está libre y qué publica Modrinth.
//!
//! Y el informe, que es el contrato de `doctor --json`: el mismo JSON, con las
//! claves en el mismo orden, para poder comparar los dos CLI con un `diff`.
//!
//! **Lo que falta**: el boot test (arrancar el server y capturar el log) y los
//! `apply` de los fixes. En modo `--json` el TS no aplica nada y el boot se puede
//! desactivar con `--no-boot`, así que el informe de solo lectura —que es el que
//! usan los scripts— sí está completo.

use forge_core::doctor::{
    dedupe_por_codigo, etiqueta_pendiente, evaluar_estaticas, evaluar_log, score_y_nota,
    siguen_rotos, ContextoEstatico, ContextoLog, Finding, Hechos, Severity, VistaServidor,
};
use forge_core::types::ServerConfig;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Timeout de `java -version`, el mismo que pone el TS (`execFile` con 15000 ms).
const JAVA_TIMEOUT: Duration = Duration::from_secs(15);

// ---------- el directorio ----------

/// `VistaServidor` sobre un directorio de verdad.
pub struct VistaDisco {
    base: PathBuf,
}

impl VistaDisco {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }

    fn ruta(&self, rel: &str) -> PathBuf {
        self.base.join(rel)
    }
}

impl VistaServidor for VistaDisco {
    fn existe(&self, rel: &str) -> bool {
        self.ruta(rel).exists()
    }

    fn leer(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.ruta(rel)).ok()
    }

    fn listar(&self, rel: &str) -> Option<Vec<String>> {
        let mut nombres: Vec<String> = std::fs::read_dir(self.ruta(rel))
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        // `readdir` de Node no garantiza orden; el doctor solo mira el primero que
        // casa con un slug conocido, así que se ordena para que sea determinista
        nombres.sort();
        Some(nombres)
    }
}

// ---------- la máquina ----------

/// `Hechos` contra el sistema y la red.
pub struct HechosReales;

impl Hechos for HechosReales {
    /// `javaMajorOf`: `java -version` y a leer la versión del texto.
    ///
    /// El TS usa `execFile`, no `exec`, así que una ruta con espacios funciona
    /// igual que aquí: los argumentos van sueltos y no pasan por el shell.
    fn java_mayor(&self, ruta_java: &str) -> Option<u32> {
        let salida = ejecutar_con_timeout(ruta_java, &["-version"], JAVA_TIMEOUT)?;
        // `stderr || stdout`: gana el stderr, y si está vacío el stdout
        let texto = if salida.stderr.trim().is_empty() {
            salida.stdout
        } else {
            salida.stderr
        };
        let re = regex_lite(r#"version "(\d+)(?:\.(\d+))?"#)?;
        let caps = re.captures(&texto)?;
        let mayor: u32 = caps.get(1)?.as_str().parse().ok()?;
        // Java 8 se anuncia como "1.8.0_402"
        if mayor == 1 {
            caps.get(2)?.as_str().parse().ok()
        } else {
            Some(mayor)
        }
    }

    fn puerto_libre(&self, puerto: u16) -> bool {
        std::net::TcpListener::bind(("0.0.0.0", puerto)).is_ok()
    }

    fn ultima_version(
        &self,
        slug: &str,
        mc: &str,
        loader: &str,
    ) -> Option<forge_core::modrinth::ModrinthVersion> {
        // el TS pasa `{ gameVersions: [mc], loaders: [loader] }`
        let juegos = vec![mc.to_string()];
        let loaders = vec![loader.to_string()];
        forge_api::modrinth::latest_version(slug, Some(&juegos), Some(&loaders)).ok()
    }
}

struct Salida {
    stdout: String,
    stderr: String,
}

/// Ejecuta un programa y recoge su salida, con deadline.
///
/// `Command` no trae timeout, así que se sondea `try_wait` y se mata al pasarse.
/// Es suficiente para `java -version`, que escribe cuatro líneas y termina; no vale
/// para un proceso que escupa megabytes por el pipe.
fn ejecutar_con_timeout(programa: &str, args: &[&str], timeout: Duration) -> Option<Salida> {
    let mut hijo = Command::new(programa)
        .args(args)
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
    Some(Salida {
        stdout: String::from_utf8_lossy(&salida.stdout).to_string(),
        stderr: String::from_utf8_lossy(&salida.stderr).to_string(),
    })
}

fn regex_lite(patron: &str) -> Option<regex::Regex> {
    regex::Regex::new(patron).ok()
}

// ---------- el informe ----------

/// El mismo JSON que escribe `doctor --json` en el TS, en el mismo orden.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Informe {
    pub findings: Vec<HallazgoJson>,
    pub applied: Vec<String>,
    pub pending: Vec<String>,
    pub score: u32,
    pub grade: &'static str,
    pub boot_tested: bool,
    pub boot_ok: bool,
}

/// Un hallazgo tal cual sale por JSON: el `fix` va anidado y **se omite** si no lo
/// hay, porque el TS escribe `undefined` y `JSON.stringify` tira la clave.
#[derive(Debug, Serialize)]
pub struct HallazgoJson {
    pub severity: &'static str,
    pub code: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixJson>,
}

/// Los metadatos del fix que ve el usuario (`apply` no se serializa nunca).
#[derive(Debug, Serialize)]
pub struct FixJson {
    pub code: String,
    pub description: String,
    pub considerable: bool,
}

fn a_json(f: &Finding) -> HallazgoJson {
    HallazgoJson {
        severity: f.severity.as_str(),
        code: f.code.clone(),
        source: f.source.clone(),
        message: f.message.clone(),
        fix: f.fix_code.as_ref().map(|code| FixJson {
            code: code.clone(),
            description: f.fix_description.clone().unwrap_or_default(),
            considerable: f.fix_considerable,
        }),
    }
}

/// Ejecuta el doctor en **solo lectura**: informa y no toca nada.
///
/// Es el camino de `doctor --json`, y el único que un script debería usar: los
/// fixes automáticos del TS cambian el server y luego puntúan un server que ya no
/// es el que se inspeccionó.
pub fn doctor_readonly(
    cfg: &ServerConfig,
    dir: &Path,
    log_lines: &[String],
    boot: (bool, bool),
) -> Informe {
    let vista = VistaDisco::new(dir);
    let hechos = HechosReales;
    let ctx = ContextoEstatico {
        mc_version: cfg.mc_version.clone(),
        port: cfg.port,
        engine: cfg.engine,
        dir: dir.to_string_lossy().to_string(),
    };

    let mut findings = evaluar_estaticas(&vista, &ctx, &hechos);
    if boot.0 {
        findings.extend(evaluar_log(
            log_lines,
            &ContextoLog {
                view_distance: cfg.view_distance,
            },
        ));
    }

    // dedupe por código conservando el primero, como el `Set` del TS
    let all = dedupe_por_codigo(&findings);

    // en solo lectura no se aplica nada, así que los fixes se listan como
    // pendientes: los automáticos con sufijo, los considerables sin él
    let applied: Vec<String> = Vec::new();
    let pending: Vec<String> = all
        .iter()
        .filter(|f| f.fix_code.is_some())
        .map(|f| {
            etiqueta_pendiente(
                f.fix_description.as_deref().unwrap_or_default(),
                f.fix_considerable,
            )
        })
        .collect();

    let (score, grade) = score_y_nota(&siguen_rotos(&all, &applied));

    Informe {
        findings: all.iter().map(a_json).collect(),
        applied,
        pending,
        score,
        grade,
        boot_tested: boot.0,
        boot_ok: boot.1,
    }
}

/// Los hallazgos por gravedad, para el resumen humano.
pub fn contar(informe: &Informe) -> (usize, usize, usize) {
    let n = |s: Severity| {
        informe
            .findings
            .iter()
            .filter(|f| f.severity == s.as_str())
            .count()
    };
    (n(Severity::Error), n(Severity::Warning), n(Severity::Info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_ocho_se_anuncia_como_uno_ocho() {
        // la regla del TS: `maj === 1 ? m[2] : maj`
        let re = regex_lite(r#"version "(\d+)(?:\.(\d+))?"#).unwrap();
        let caps = re.captures(r#"openjdk version "1.8.0_402""#).unwrap();
        assert_eq!(&caps[1], "1");
        assert_eq!(&caps[2], "8");

        let caps = re
            .captures(r#"openjdk version "21.0.2" 2024-01-16"#)
            .unwrap();
        assert_eq!(&caps[1], "21");
        assert!(caps.get(2).is_some());
    }

    #[test]
    fn un_puerto_ocupado_no_esta_libre() {
        let hechos = HechosReales;
        let srv = std::net::TcpListener::bind(("0.0.0.0", 0)).expect("un puerto efímero");
        let puerto = srv.local_addr().unwrap().port();
        assert!(
            !hechos.puerto_libre(puerto),
            "el que acabamos de abrir no puede estar libre"
        );
        drop(srv);
        assert!(hechos.puerto_libre(puerto), "al soltarlo vuelve a estarlo");
    }

    #[test]
    fn una_vista_sin_directorio_no_lista_nada() {
        let vista = VistaDisco::new(std::env::temp_dir().join("openblock-no-existe-jamas"));
        assert!(!vista.existe("server.properties"));
        assert!(vista.leer("server.properties").is_none());
        assert!(vista.listar("plugins").is_none());
    }
}
