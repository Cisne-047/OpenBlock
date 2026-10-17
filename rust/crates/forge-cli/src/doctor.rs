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

// ---------- los arreglos ----------

/// Cómo se comporta el doctor: solo informar, o informar y arreglar lo seguro.
///
/// En el TS es `readOnly`, que sale de `--json` o de no tener TTY. Los fixes
/// **considerables** (los que cambian el server de forma visible) no se aplican sin
/// que alguien los confirme; aquí no hay TTY todavía, así que se listan como
/// pendientes igual que en el TS con `--yes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    SoloLectura,
    Aplicar,
}

/// Reescribe claves de `server.properties` (`patchProperties`).
///
/// Dos detalles del TS que se reproducen:
///
/// - La clave va **sin escapar** en el regex, así que `query.port` también casa con
///   `queryXport`. Es el mismo tipo de quirk que el prefijo del maven de Forge.
/// - `.*` de JS no cruza el `\r`, y aquí se traduce como `[^\r\n]*` por eso: un
///   `server.properties` con finales CRLF conserva el `\r` y el fichero sigue
///   teniendo los mismos finales de línea.
///
/// Y solo sustituye la **primera** aparición (el `replace` del TS va sin `/g`).
pub fn patch_properties(dir: &Path, patch: &[(&str, String)]) -> std::io::Result<()> {
    let p = dir.join("server.properties");
    let mut txt = std::fs::read_to_string(&p).unwrap_or_default();
    for (k, v) in patch {
        let re = regex::Regex::new(&format!("(?m)^{k}=[^\\r\\n]*"))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        let nuevo = format!("{k}={v}");
        txt = if re.is_match(&txt) {
            // `NoExpand`: el `$` de una contraseña no es una referencia a un grupo
            re.replace(&txt, regex::NoExpand(&nuevo)).to_string()
        } else {
            // sin comprobar el salto final: si el fichero no acaba en `\n`, la clave
            // nueva se pega a la última línea, igual que en el TS
            format!("{txt}{nuevo}\n")
        };
    }
    std::fs::write(&p, txt)
}

/// Reescribe el heap en los ficheros de flags que existan (`patchRam`).
pub fn patch_ram(dir: &Path, ram_mb: u32) -> std::io::Result<()> {
    for f in ["jvm-args.txt", "user_jvm_args.txt"] {
        let p = dir.join(f);
        if let Ok(txt) = std::fs::read_to_string(&p) {
            std::fs::write(&p, forge_core::doctor::rewrite_heap_flags(&txt, ram_mb))?;
        }
    }
    Ok(())
}

/// Guarda la config en `openblock.json` (`writeOpenblock`).
///
/// Es el mismo JSON que lee el doctor, así que el round-trip tiene que ser exacto:
/// por eso el orden de los campos del struct del núcleo es el del TS.
pub fn write_openblock(dir: &Path, cfg: &ServerConfig) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(dir.join("openblock.json"), format!("{json}\n"))
}

/// Los primeros 50 puertos desde `start` que estén libres (`findFreePort`).
pub fn find_free_port(start: u16) -> Result<u16, String> {
    let hechos = HechosReales;
    for p in start..start.saturating_add(50) {
        if hechos.puerto_libre(p) {
            return Ok(p);
        }
    }
    Err("Sin puertos libres".to_string())
}

/// Una contraseña RCON nueva, con la forma de la del TS: 14 caracteres base36.
///
/// `Math.random()` no se puede reproducir —ni falta: el valor es aleatorio en los
/// dos lados—, así que lo que se reproduce es la **forma**: 8 + 6 caracteres del
/// alfabeto de `toString(36)`.
pub fn password_aleatorio() -> String {
    const ALFABETO: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut semilla = nanos ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    let mut salida = String::with_capacity(14);
    for _ in 0..14 {
        // xorshift64, el mismo que usa el benchmark
        semilla ^= semilla << 13;
        semilla ^= semilla >> 7;
        semilla ^= semilla << 17;
        salida.push(ALFABETO[(semilla % 36) as usize] as char);
    }
    salida
}

/// Aplica el fix de un hallazgo. Devuelve el código si se aplicó.
///
/// `None` significa que no se aplicó: o no hay fix, o es de los que todavía no
/// están portados (`install-java` necesita bajar un JDK y `update-plugin` bajar el
/// jar), y en ese caso el hallazgo se queda en `pending` en vez de desaparecer.
fn aplicar_uno(f: &Finding, cfg: &mut ServerConfig, dir: &Path) -> Option<String> {
    let code = f.fix_code.as_deref()?;
    let ok = match code {
        "accept-eula" => std::fs::write(dir.join("eula.txt"), "eula=true\n").is_ok(),
        "sync-chunk-writes-off" => {
            patch_properties(dir, &[("sync-chunk-writes", "false".to_string())]).is_ok()
        }
        "change-port" => {
            // el fix del log y el de la regla estática **no** parchean lo mismo: el
            // del log mueve también el puerto de RCON, el estático no
            match find_free_port(cfg.port.saturating_add(1)) {
                Ok(puerto) => {
                    let mut patch = vec![
                        ("server-port", puerto.to_string()),
                        ("query.port", puerto.to_string()),
                    ];
                    if f.source == "log" {
                        patch.push(("rcon.port", puerto.saturating_add(10).to_string()));
                    }
                    cfg.port = puerto;
                    patch_properties(dir, &patch).is_ok() && write_openblock(dir, cfg).is_ok()
                }
                Err(_) => false,
            }
        }
        "raise-ram" => {
            // `Math.round(ramMB * 1.5)`
            let ram = ((cfg.ram_mb as f64) * 1.5).round() as u32;
            cfg.ram_mb = ram;
            patch_ram(dir, ram).is_ok() && write_openblock(dir, cfg).is_ok()
        }
        "lower-vd" => {
            let vd = cfg.view_distance.saturating_sub(2).max(4);
            let sd = cfg.simulation_distance.saturating_sub(1).max(4);
            cfg.view_distance = vd;
            cfg.simulation_distance = sd;
            patch_properties(
                dir,
                &[
                    ("view-distance", vd.to_string()),
                    ("simulation-distance", sd.to_string()),
                ],
            )
            .is_ok()
                && write_openblock(dir, cfg).is_ok()
        }
        "rcon-password" => {
            let np = password_aleatorio();
            cfg.rcon.password = np.clone();
            patch_properties(dir, &[("rcon.password", np)]).is_ok()
                && write_openblock(dir, cfg).is_ok()
        }
        "disable-plugin" => {
            // el nombre sale del match del log, que es el `fix_arg` del hallazgo
            let nombre = f.fix_args.first()?;
            let plugins = dir.join("plugins");
            let destino = dir.join("plugins_disabled");
            if std::fs::create_dir_all(&destino).is_err() {
                return None;
            }
            let mut movido = true;
            if let Ok(entradas) = std::fs::read_dir(&plugins) {
                for e in entradas.flatten() {
                    let fichero = e.file_name().to_string_lossy().to_string();
                    if fichero.to_lowercase().contains(&nombre.to_lowercase())
                        && fichero.ends_with(".jar")
                    {
                        movido =
                            std::fs::rename(e.path(), destino.join(&fichero)).is_ok() && movido;
                    }
                }
            }
            movido
        }
        "update-plugin" => {
            // los datos salen de Modrinth y los trae el hallazgo: url, nombre y
            // (si lo hay) sha512 de la version nueva
            let (Some(url), Some(nombre)) = (f.fix_args.first(), f.fix_args.get(1)) else {
                return None;
            };
            let sha512 = f.fix_args.get(2).map(String::as_str);
            // el jar viejo se va antes de bajar el nuevo, como en el TS
            let _ =
                std::fs::remove_file(dir.join("plugins").join(f.source.replace("plugins/", "")));
            let destino = dir.join("plugins").join(nombre);
            forge_api::net::download_to(
                url,
                &destino,
                &forge_api::net::OpcionesDescarga {
                    sha512,
                    ..Default::default()
                },
            )
            .is_ok()
        }
        // pendiente de portar: necesita bajar un JDK (ensureJava)
        _ => false,
    };
    if ok {
        Some(code.to_string())
    } else {
        None
    }
}

/// Ejecuta el doctor entero: estáticas, boot, log, dedupe, fixes y score.
///
/// Sigue el orden del TS, que importa: las estáticas y sus fixes seguros van
/// **antes** del boot test, para que el server arranque ya con la EULA aceptada y
/// sin `sync-chunk-writes`. Los fixes considerables no se aplican sin confirmación.
pub fn ejecutar(cfg: &mut ServerConfig, dir: &Path, boot: bool, modo: Modo) -> Informe {
    let aplicar = modo == Modo::Aplicar;
    let vista = VistaDisco::new(dir);
    let hechos = HechosReales;
    let ctx = ContextoEstatico {
        mc_version: cfg.mc_version.clone(),
        port: cfg.port,
        engine: cfg.engine,
        dir: dir.to_string_lossy().to_string(),
    };

    let mut applied: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();

    let mut findings = evaluar_estaticas(&vista, &ctx, &hechos);

    // 1) los fixes seguros de las estáticas, ya, para que el boot arranque limpio
    if aplicar {
        for f in &findings {
            if f.fix_code.is_some() && !f.fix_considerable {
                if let Some(code) = aplicar_uno(f, cfg, dir) {
                    applied.push(code);
                }
            }
        }
    }

    // 2) boot test (dry run real)
    let mut lineas: Vec<String> = Vec::new();
    let boot_ok = if boot {
        boot_test(cfg, dir, &mut |l| lineas.push(l))
    } else {
        false
    };

    // 3) reglas de log sobre lo capturado
    if boot {
        findings.extend(evaluar_log(
            &lineas,
            &ContextoLog {
                view_distance: cfg.view_distance,
            },
        ));
    }

    // dedupe por código: gana el primero, y las estáticas van antes
    let all = dedupe_por_codigo(&findings);

    // 4) los fixes que quedan: seguros al instante, considerables a `pending`
    for f in &all {
        let Some(code) = f.fix_code.as_deref() else {
            continue;
        };
        if applied.iter().any(|c| c == code) {
            continue;
        }
        let etiqueta = etiqueta_pendiente(
            f.fix_description.as_deref().unwrap_or_default(),
            f.fix_considerable,
        );
        if !aplicar || f.fix_considerable {
            pending.push(etiqueta);
        } else if let Some(aplicado) = aplicar_uno(f, cfg, dir) {
            applied.push(aplicado);
        } else {
            // no se pudo (o no está portado): se lista, no se hace como si se hubiera
            // arreglado
            pending.push(etiqueta);
        }
    }

    // 5) el score refleja el estado POST-fix: solo cuenta lo que sigue roto
    let unique = siguen_rotos(&all, &applied);
    let (score, grade) = score_y_nota(&unique);

    Informe {
        findings: unique.iter().map(a_json).collect(),
        applied,
        pending,
        score,
        grade,
        boot_tested: boot,
        boot_ok,
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

    fn dir_de_prueba(nombre: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("openblock-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("directorio de prueba");
        d
    }

    #[test]
    fn patch_properties_reproduce_los_detalles_del_ts() {
        let dir = dir_de_prueba("patch");

        // 1) la clave que esta se sustituye, y el `\r` de CRLF se queda
        std::fs::write(
            dir.join("server.properties"),
            "motd=hola\r\nsync-chunk-writes=true\r\nview-distance=8\r\n",
        )
        .unwrap();
        patch_properties(&dir, &[("sync-chunk-writes", "false".to_string())]).unwrap();
        let txt = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        assert_eq!(
            txt,
            "motd=hola\r\nsync-chunk-writes=false\r\nview-distance=8\r\n"
        );

        // 2) la que no esta se anade al final, con su salto
        patch_properties(&dir, &[("enable-rcon", "true".to_string())]).unwrap();
        let txt = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        assert!(
            txt.ends_with("view-distance=8\r\nenable-rcon=true\n"),
            "{txt:?}"
        );

        // 3) la clave va SIN escapar: `query.port` casa con `queryXport`, y gana la
        //    primera aparicion del fichero (el mismo quirk que el maven de Forge)
        std::fs::write(
            dir.join("server.properties"),
            "queryXport=9\nquery.port=1\n",
        )
        .unwrap();
        patch_properties(&dir, &[("query.port", "25566".to_string())]).unwrap();
        let txt = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        assert_eq!(
            txt, "query.port=25566\nquery.port=1\n",
            "el punto es un comodin: renombra la linea"
        );

        // 4) y de la misma clave solo se cambia la primera
        std::fs::write(
            dir.join("server.properties"),
            "server-port=1\nserver-port=2\n",
        )
        .unwrap();
        patch_properties(&dir, &[("server-port", "25567".to_string())]).unwrap();
        let txt = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        assert_eq!(txt, "server-port=25567\nserver-port=2\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_contrasena_tiene_la_forma_de_la_del_ts() {
        // 8 + 6 caracteres base36; el valor es aleatorio en los dos lados
        for _ in 0..20 {
            let p = password_aleatorio();
            assert_eq!(p.len(), 14, "{p}");
            assert!(
                p.chars()
                    .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()),
                "{p} deberia ser base36"
            );
        }
    }

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

#[cfg(test)]
mod tests_de_los_fixes {
    use super::*;
    use forge_core::doctor::{Finding, Severity};

    /// El `update-plugin` de verdad: borra el jar viejo y baja el nuevo desde la
    /// url que trae el hallazgo, con su sha512. Se prueba con un servidor local
    /// porque la alternativa es esperar a que Modrinth publique una version nueva.
    #[test]
    fn update_plugin_borra_el_viejo_y_baja_el_nuevo() {
        use std::io::{Read, Write};

        let contenido = b"jar nuevo".to_vec();
        let contenido_servidor = contenido.clone();
        let sha512 = {
            use sha2::Digest;
            let mut h = String::new();
            for b in sha2::Sha512::digest(&contenido) {
                h.push_str(&format!("{b:02x}"));
            }
            h
        };
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = listener.local_addr().unwrap().port();
        let hilo = std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf);
                let cabecera = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    contenido.len()
                );
                let _ = sock.write_all(cabecera.as_bytes());
                let _ = sock.write_all(&contenido_servidor);
            }
        });

        let dir = std::env::temp_dir().join(format!("openblock-fix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("plugins")).unwrap();
        std::fs::write(dir.join("plugins/spark-1.0.jar"), "viejo").unwrap();

        let mut cfg = config_de_prueba();
        let mut f = Finding::new(Severity::Warning, "PLUGIN_UPDATE");
        f.source = "plugins/spark-1.0.jar".to_string();
        f.fix_code = Some("update-plugin".to_string());
        f.fix_args = vec![
            format!("http://127.0.0.1:{puerto}/spark-2.0.jar"),
            "spark-2.0.jar".to_string(),
            sha512.clone(),
        ];

        assert_eq!(
            aplicar_uno(&f, &mut cfg, &dir).as_deref(),
            Some("update-plugin")
        );
        let _ = hilo.join();
        assert!(
            !dir.join("plugins/spark-1.0.jar").exists(),
            "el viejo se va"
        );
        assert_eq!(
            std::fs::read(dir.join("plugins/spark-2.0.jar")).unwrap(),
            b"jar nuevo".to_vec()
        );

        // y con un hash que no es el suyo, no deja el fichero bueno por bueno: falla
        let mut f2 = f.clone();
        f2.fix_args[2] = "0".repeat(128);
        assert!(
            aplicar_uno(&f2, &mut cfg, &dir).is_none(),
            "un hash malo no puede pasar"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn config_de_prueba() -> ServerConfig {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/parity-config.json");
        serde_json::from_str(&std::fs::read_to_string(p).expect("la config de paridad"))
            .expect("config válida")
    }
}
