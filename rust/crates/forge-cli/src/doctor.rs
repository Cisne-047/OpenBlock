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
use forge_core::types::{EngineId, ServerConfig};
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

// ---------- boot test ----------

/// Cuánto se espera a que el server diga «Done», y cada cuánto se mira.
const BOOT_DEADLINE: Duration = Duration::from_secs(180);
const BOOT_POLL: Duration = Duration::from_millis(2500);
/// Lo que se espera tras pedir `stop` por RCON antes de matarlo.
const RCON_ESPERA: Duration = Duration::from_secs(8);
/// Lo que se espera tras el kill antes de insistir.
const KILL_ESPERA: Duration = Duration::from_secs(3);

/// Arranca el server de verdad y mira si llega a `Done` (`bootTest`).
///
/// Captura el log por dos vías, como el TS: lo que el proceso escribe por
/// stdout/stderr **y** lo que aparece al final de `logs/latest.log`. Se solapan, y
/// da igual: las reglas de log usan el primer match de cada patrón.
///
/// Devuelve `true` si el server arrancó. El apagado es limpio por RCON si está
/// habilitado y el server llegó a arrancar; si no, se mata.
///
/// # El fallo del original que se reproduce
///
/// `antes` es el tamaño del log **anterior** y la cola se lee a partir de ahí. Paper
/// **rota** `latest.log` al arrancar (lo comprime a `AAAA-MM-DD-N.log.gz` y empieza
/// uno nuevo), así que a partir del segundo diagnóstico sobre el mismo directorio el
/// fichero nuevo es más corto que `antes`: no se lee la cola, no se ve el `Done` y
/// el informe dice que el server no arrancó. Y como `done` es falso, tampoco se
/// apaga por RCON — se mata, sin guardar el mundo.
///
/// Se reproduce tal cual: el puerto no arregla el original. Está comprobado que los
/// dos CLI dicen exactamente lo mismo en ese caso, que es lo que hace que la
/// afirmación sea verificable y no una sospecha.
pub fn boot_test(cfg: &ServerConfig, dir: &Path, capture: &mut dyn FnMut(String)) -> bool {
    let path_file = dir.join("runtime").join("java.path");
    let java_path = match std::fs::read_to_string(&path_file) {
        Ok(t) => t.trim().to_string(),
        Err(_) => "java".to_string(),
    };

    // `readFileSync(jvmFile).split(/\s+/).filter(Boolean)`
    let args: Vec<String> = match std::fs::read_to_string(dir.join("jvm-args.txt")) {
        Ok(t) => t.split_whitespace().map(str::to_string).collect(),
        Err(_) => Vec::new(),
    };

    let runner = if cfg!(windows) { "run.bat" } else { "run.sh" };
    let use_runner = dir.join(runner).exists();

    let log_path = dir.join("logs").join("latest.log");
    // el TS cuenta unidades UTF-16 y aquí bytes; la diferencia solo aparece si el
    // log ya tenía texto multibyte, y en ese caso los dos leen de más, no de menos
    let antes = std::fs::read(&log_path).map(|b| b.len()).unwrap_or(0);

    let mut cmd = if use_runner {
        let mut c = Command::new(if cfg!(windows) { "cmd.exe" } else { "bash" });
        if cfg!(windows) {
            c.arg("/c");
        }
        c.arg(runner).arg("nogui");
        c
    } else {
        let mut c = Command::new(&java_path);
        c.args(&args).arg("-jar");
        c.arg(
            forge_core::doctor::jar_name_from_start_script(&VistaDisco::new(dir))
                .unwrap_or_else(|| "server.jar".to_string()),
        );
        c.arg(if cfg.engine == EngineId::Vanilla {
            "nogui"
        } else {
            "--nogui"
        });
        c
    };

    let mut hijo = match cmd
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(h) => h,
        Err(e) => {
            capture(format!("No se pudo arrancar el server: {e}"));
            return false;
        }
    };

    // los dos pipes se leen en hilos: si no, el server se queda bloqueado cuando
    // llena el buffer y nunca llega a `Done`
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    for pipe in [
        hijo.stdout.take().map(Pipe::Out),
        hijo.stderr.take().map(Pipe::Err),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let lector: Box<dyn std::io::Read + Send> = match pipe {
                Pipe::Out(s) => Box::new(s),
                Pipe::Err(s) => Box::new(s),
            };
            let mut lector = std::io::BufReader::new(lector);
            let mut linea = String::new();
            while std::io::BufRead::read_line(&mut lector, &mut linea).unwrap_or(0) > 0 {
                let limpia = linea.trim();
                if !limpia.is_empty() && tx.send(limpia.to_string()).is_err() {
                    break;
                }
                linea.clear();
            }
        });
    }
    drop(tx);

    let mut done = false;
    let limite = Instant::now() + BOOT_DEADLINE;
    while Instant::now() < limite {
        std::thread::sleep(BOOT_POLL);
        // lo que el proceso haya escrito por sus pipes
        while let Ok(l) = rx.try_recv() {
            capture(l);
        }
        // `child.exitCode !== null || child.signalCode`
        if matches!(hijo.try_wait(), Ok(Some(_))) {
            break;
        }
        // y lo que haya crecido el log
        if let Ok(txt) = std::fs::read(&log_path) {
            if txt.len() > antes {
                let cola = String::from_utf8_lossy(&txt[antes..]).to_string();
                for l in cola.split('\n') {
                    if !l.trim().is_empty() {
                        capture(l.trim().to_string());
                    }
                }
                if regex_lite(r"Done \([0-9.]+s\)!").is_some_and(|re| re.is_match(&cola)) {
                    done = true;
                    break;
                }
                if regex_lite(r#"Failed to start the minecraft server|Exception in thread "main""#)
                    .is_some_and(|re| re.is_match(&cola))
                {
                    break;
                }
            }
        }
    }

    // lo que quedara por leer, para que las reglas vean el final del arranque
    while let Ok(l) = rx.try_recv() {
        capture(l);
    }

    // apagado: limpio por RCON si está arriba, kill si no
    if done && cfg.rcon.enabled {
        let _ = crate::rcon::with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
            r.command("stop")
        });
        std::thread::sleep(RCON_ESPERA);
    }
    if matches!(hijo.try_wait(), Ok(None)) {
        // el TS manda SIGTERM y luego SIGKILL; `kill` de Rust es SIGKILL en Unix y
        // TerminateProcess en Windows, así que aquí no hay apagado suave
        let _ = hijo.kill();
        std::thread::sleep(KILL_ESPERA);
        if matches!(hijo.try_wait(), Ok(None)) {
            let _ = hijo.kill();
        }
    }
    done
}

enum Pipe {
    Out(std::process::ChildStdout),
    Err(std::process::ChildStderr),
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
