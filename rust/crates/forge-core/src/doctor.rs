//! La parte pura del doctor, portada de `src/doctor.ts`.
//!
//! El doctor completo necesita red, disco, procesos y RCON, así que va al CLI. Pero
//! su **aritmética** —el score, las notas, el dedupe y la etiqueta de pendiente— es
//! pura, y es justo donde un puerto se equivoca en silencio: un score mal calculado
//! no da error, da un informe que miente sobre el estado del server.
//!
//! Aquí viven también las **reglas de log** como datos y su evaluación: son texto
//! de entrada y hallazgos de salida, sin tocar nada. Lo que se queda en el CLI es
//! el `apply` de cada fix, que sí escribe ficheros y arranca procesos.
//!
//! Las **reglas estáticas** son puras por el mismo motivo, pero necesitan mirar el
//! directorio y preguntar dos o tres cosas a la máquina: eso entra por dos traits
//! (`VistaServidor` y `Hechos`), así que el núcleo sigue sin tocar el disco ni
//! arrancar un proceso.

use crate::modrinth::ModrinthVersion;
use crate::types::EngineId;

/// Gravedad de un hallazgo (`Severity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    /// Parsea una gravedad; `None` si no es conocida.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "error" => Self::Error,
            "warning" => Self::Warning,
            "info" => Self::Info,
            _ => return None,
        })
    }

    /// Lo que resta al score: cada error 20, cada warning 5, y `info` **cero**.
    pub fn peso(self) -> u32 {
        match self {
            Self::Error => 20,
            Self::Warning => 5,
            Self::Info => 0,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Un hallazgo del diagnóstico (`Finding`).
///
/// Lleva los metadatos del fix —**código y si es considerable**— pero no su
/// `apply`: la closure vive en el CLI, que es donde están los efectos. Es la misma
/// separación que hizo el TS al proyectar el `fix` para el JSON del doctor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// Identificador estable de la regla (`EULA_FILE`, `PLUGIN_UPDATE`…).
    pub code: String,
    pub message: String,
    /// De dónde sale: `"log"`, `"server.properties"`…
    pub source: String,
    /// Código del fix que lo arregla, si lo hay (`install-java`, `accept-eula`…).
    pub fix_code: Option<String>,
    /// `true` = hay que preguntar antes de aplicarlo.
    pub fix_considerable: bool,
    /// Descripción del fix, para el informe.
    pub fix_description: Option<String>,
}

impl Finding {
    /// Constructor corto para los tests: hallazgo sin fix.
    pub fn new(severity: Severity, code: &str) -> Self {
        Self {
            severity,
            code: code.to_string(),
            message: String::new(),
            source: String::new(),
            fix_code: None,
            fix_considerable: false,
            fix_description: None,
        }
    }

    /// Hallazgo con fix, que es lo que distingue si resta al score tras aplicarlo.
    pub fn con_fix(severity: Severity, code: &str, fix_code: &str, considerable: bool) -> Self {
        Self {
            fix_code: Some(fix_code.to_string()),
            fix_considerable: considerable,
            fix_description: Some(format!("descripción de {fix_code}")),
            ..Self::new(severity, code)
        }
    }
}

/// El score de salud y su nota, a partir de los hallazgos que **siguen** rotos
/// (`runDoctor`, el bloque del score).
///
/// El TS calcula sobre los findings post-fix: `Math.max(0, 100 - errors*20 -
/// warnings*5)`. El `max(0)` es un tope inferior —con 5 errores ya se llega a 0—
/// y no hay tope superior porque el score no puede pasar de 100 sin hallazgos.
///
/// `info` **no resta**: es informativo y se cuenta para el informe, no para la nota.
/// Confundirlo con un warning bajaría el score 5 puntos por algo que no está mal.
pub fn score_y_nota(findings: &[Finding]) -> (u32, &'static str) {
    let resta: u32 = findings.iter().map(|f| f.severity.peso()).sum();
    let score = 100u32.saturating_sub(resta);
    (score, nota(score))
}

/// La nota del score (`grade`), con los cortes del TS.
///
/// `>= 90` SANO · `>= 70` OK CON AVISOS · `>= 40` NECESITA ATENCIÓN · resto CRÍTICO.
/// Los cortes están en tres sitios del TS —aquí, en el color de la barra y en el
/// `exitCode` del CLI— y solo uno es la fuente: este.
pub fn nota(score: u32) -> &'static str {
    if score >= 90 {
        "SANO"
    } else if score >= 70 {
        "OK CON AVISOS"
    } else if score >= 40 {
        "NECESITA ATENCIÓN"
    } else {
        "CRÍTICO"
    }
}

/// Quita hallazgos con el mismo código, quedándose con el **primero**
/// (`runDoctor`, el dedupe).
///
/// El TS usa un `Set` y un `filter`: gana el primero que aparece y el orden se
/// conserva, así que las reglas estáticas —que se recogen antes— van antes que las
/// de log. Ese orden es el que ve el usuario en el informe.
pub fn dedupe_por_codigo(findings: &[Finding]) -> Vec<Finding> {
    let mut vistos = std::collections::HashSet::new();
    findings
        .iter()
        .filter(|f| vistos.insert(f.code.as_str()))
        .cloned()
        .collect()
}

/// Cómo se lista un fix que no se ha aplicado (`runDoctor` en modo solo lectura).
///
/// El TS distingue los automáticos con el sufijo `" (automático)"`, para que el
/// informe diga qué se habría arreglado solo sin que el score mienta sobre el
/// estado inspeccionado. Un considerable se lista sin sufijo, porque ese sí
/// necesita una decisión.
pub fn etiqueta_pendiente(descripcion: &str, considerable: bool) -> String {
    if considerable {
        descripcion.to_string()
    } else {
        format!("{descripcion} (automático)")
    }
}

/// Filtra los hallazgos cuyo fix **sigue sin aplicarse** (`runDoctor`: `unique`).
///
/// El TS hace `!(f.fix && applied.includes(f.fix.code))`: un hallazgo sin fix nunca
/// se filtra —no hay nada que aplicar, así que sigue roto— y uno con fix se filtra
/// solo si su código está en `applied`. Un hallazgo sin fix cuenta para el score
/// aunque el resto se haya arreglado; por eso el `ENGINE_JAR_MISSING` del self-test
/// sigue restando y da 80.
pub fn siguen_rotos(findings: &[Finding], codigos_aplicados: &[String]) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| match &f.fix_code {
            None => true,
            Some(c) => !codigos_aplicados.contains(c),
        })
        .cloned()
        .collect()
}

// ---------- reglas sobre el log del boot ----------

/// Lo que la config aporta a las descripciones de los fixes de log.
///
/// De todo el `ServerConfig`, las reglas de log solo leen `viewDistance` (para decir
/// a cuánto bajaría). Se pasa lo mínimo en vez de la config entera para que la
/// evaluación no dependa de nada más.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextoLog {
    pub view_distance: u32,
}

/// Los grupos capturados del primer match de una regla (`RegExpMatchArray`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capturas(pub Vec<Option<String>>);

impl Capturas {
    /// Un grupo por índice. Ausente es `"undefined"`, que es lo que escribiría el
    /// TS al meter un grupo sin casar dentro de una plantilla.
    pub fn grupo(&self, i: usize) -> &str {
        match self.0.get(i).and_then(|g| g.as_deref()) {
            Some(s) => s,
            None => "undefined",
        }
    }
}

/// Los metadatos de un fix: **sin** el `apply`, que es un efecto y vive en el CLI.
pub struct FixSpec {
    pub code: &'static str,
    /// `true` = hay que preguntar antes de aplicarlo.
    pub considerable: bool,
    /// La descripción puede depender del match (el número de Java, el nombre del
    /// plugin) y de la config (a cuánto bajar la view-distance).
    pub description: fn(&Capturas, &ContextoLog) -> String,
}

/// Una regla sobre el log del arranque (`LogRule`).
pub struct LogRule {
    pub code: &'static str,
    pub severity: Severity,
    /// El patrón como texto. El `\w` del TS es ASCII, así que la clase va escrita a
    /// mano donde aparece; lo demás es igual.
    pub pattern: &'static str,
    pub message: &'static str,
    pub fix: Option<FixSpec>,
}

/// Las reglas sobre el log, en su orden.
///
/// El `minCount` que declara la interfaz del TS **no se usa** en ninguna parte: una
/// sola línea que case ya dispara el hallazgo. No se porta como campo porque no es
/// comportamiento, y ponerlo daría a entender que filtra.
pub const LOG_RULES: &[LogRule] = &[
    LogRule {
        code: "JAVA_VERSION",
        severity: Severity::Error,
        pattern: r"requires running the server with Java (\d+)",
        message: "Versión de Java incorrecta para esta versión de MC.",
        fix: Some(FixSpec {
            code: "install-java",
            considerable: false,
            description: |m, _| {
                format!(
                    "Descargar Temurin JDK {} y reescribir runtime/java.path",
                    m.grupo(1)
                )
            },
        }),
    },
    LogRule {
        code: "EULA_NOT_ACCEPTED",
        severity: Severity::Error,
        pattern: r"You need to agree to the EULA",
        message: "EULA sin aceptar: el server no arranca.",
        fix: Some(FixSpec {
            code: "accept-eula",
            considerable: false,
            description: |_, _| "Aceptar EULA (eula=true)".to_string(),
        }),
    },
    LogRule {
        code: "PORT_IN_USE",
        severity: Severity::Error,
        pattern: r"Failed to bind to port|Address already in use",
        message: "El puerto está ocupado por otro proceso.",
        fix: Some(FixSpec {
            code: "change-port",
            considerable: true,
            description: |_, _| {
                "Cambiar el puerto a uno libre (server.properties + openblock.json)".to_string()
            },
        }),
    },
    LogRule {
        code: "OOM",
        severity: Severity::Error,
        pattern: r"OutOfMemoryError|There is insufficient memory for the Java Runtime",
        message: "La JVM se quedó sin memoria con la RAM asignada.",
        fix: Some(FixSpec {
            code: "raise-ram",
            considerable: true,
            description: |_, _| "Subir -Xmx un 50% (jvm-args.txt + user_jvm_args.txt)".to_string(),
        }),
    },
    LogRule {
        code: "PLUGIN_CRASH",
        severity: Severity::Error,
        // el `([\w-]+)` del TS: `\w` es ASCII, así que la clase se escribe a mano
        pattern: r"Error occurred while enabling ([A-Za-z0-9_-]+)",
        message: "Un plugin revienta al cargar.",
        fix: Some(FixSpec {
            code: "disable-plugin",
            considerable: true,
            description: |m, _| format!("Mover {}.jar a plugins_disabled/", m.grupo(1)),
        }),
    },
    LogRule {
        code: "TICK_BEHIND",
        severity: Severity::Warning,
        pattern: r"Can't keep up!|Running \d+ms or \d+ ticks behind",
        message: "El server no da los 20 TPS durante el arranque.",
        fix: Some(FixSpec {
            code: "lower-vd",
            considerable: true,
            description: |_, cfg| {
                // `Math.max(4, viewDistance - 2)`, con la resta saturante del TS
                // para los valores por debajo de 2 (que dan igualmente 4)
                let nueva = cfg.view_distance.saturating_sub(2).max(4);
                format!(
                    "Bajar view-distance de {} a {} (gameplay visible)",
                    cfg.view_distance, nueva
                )
            },
        }),
    },
    LogRule {
        code: "WORLD_UPGRADE",
        severity: Severity::Warning,
        pattern: r"Upgrading level|Data fixer for",
        message: "El mundo se va a migrar a la nueva versión (proceso largo e irreversible).",
        fix: None,
    },
    LogRule {
        code: "PLUGIN_API_OLD",
        severity: Severity::Info,
        pattern: r"Unsupported API version",
        message: "Un plugin fue compilado para una API más antigua; suele funcionar igual.",
        fix: None,
    },
    LogRule {
        code: "DATAPACK_FAIL",
        severity: Severity::Error,
        pattern: r"Failed to (?:load|parse) (?:data )?pack",
        message: "Un datapack del mundo está roto.",
        fix: None,
    },
];

/// Evalúa las reglas de log sobre lo capturado (`evaluateLogRules`).
///
/// Se usa **el primer match** de cada regla: dos líneas que disparen
/// `JAVA_VERSION` dan un hallazgo, con el número de la primera. El orden de salida
/// es el de `LOG_RULES`, no el de las líneas, y una misma línea puede disparar
/// varias reglas porque cada una barre el log por su cuenta.
pub fn evaluar_log(lines: &[String], ctx: &ContextoLog) -> Vec<Finding> {
    let mut out = Vec::new();
    for rule in LOG_RULES {
        let re = regex::Regex::new(rule.pattern).expect("los patrones son constantes y válidos");
        let Some(caps) = lines.iter().find_map(|l| re.captures(l)) else {
            continue;
        };
        let capturas = Capturas(
            caps.iter()
                .map(|g| g.map(|m| m.as_str().to_string()))
                .collect(),
        );
        let fix = rule.fix.as_ref().map(|f| (f, capturas));
        out.push(Finding {
            severity: rule.severity,
            code: rule.code.to_string(),
            message: rule.message.to_string(),
            source: "log".to_string(),
            fix_code: fix.as_ref().map(|(f, _)| f.code.to_string()),
            fix_considerable: fix.as_ref().is_some_and(|(f, _)| f.considerable),
            fix_description: fix.as_ref().map(|(f, m)| (f.description)(m, ctx)),
        });
    }
    out
}

// ---------- reglas estáticas (ficheros + entorno) ----------

/// Lo que las reglas estáticas leen del directorio del server.
///
/// Es un trait y no `std::fs` porque el núcleo no toca el disco: así las reglas son
/// comparables contra el TS con un directorio de mentira, y el CLI pone la
/// implementación de verdad.
pub trait VistaServidor {
    /// `existsSync`
    fn existe(&self, rel: &str) -> bool;
    /// `readFile`. `None` si no se puede leer.
    fn leer(&self, rel: &str) -> Option<String>;
    /// `readdir`. `None` si el directorio no está.
    fn listar(&self, rel: &str) -> Option<Vec<String>>;
}

/// Lo que las reglas preguntan a la máquina o a la red.
///
/// Son los tres puntos que en el TS se inyectan en el contexto
/// (`javaMajorOf`, `isPortFree`, `latestVersion`) para que el resultado no dependa
/// del java del PATH, de los puertos ocupados ni de lo que Modrinth publique hoy.
pub trait Hechos {
    /// `javaMajorOf(javaPath)`; `None` = no se pudo averiguar.
    fn java_mayor(&self, ruta_java: &str) -> Option<u32>;
    /// `isPortFree(puerto)`.
    fn puerto_libre(&self, puerto: u16) -> bool;
    /// `latestVersion(slug, …)`; `None` donde el TS lanzaría y el `catch` lo traga.
    fn ultima_version(&self, slug: &str, mc: &str, loader: &str) -> Option<ModrinthVersion>;
}

/// Lo que las reglas estáticas leen de la config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextoEstatico {
    pub mc_version: String,
    pub port: u16,
    pub engine: EngineId,
    /// El directorio como se le enseña al usuario: sale tal cual en el `source` de
    /// `ENGINE_JAR_MISSING`, así que el port tiene que pasar el mismo que el TS.
    pub dir: String,
}

/// Una regla estática (`StaticRule`), sin el `apply` de su fix.
pub struct StaticRule {
    pub code: &'static str,
    pub check: fn(&dyn VistaServidor, &ContextoEstatico, &dyn Hechos) -> Option<Finding>,
}

/// Las reglas estáticas, en su orden.
pub const STATIC_RULES: &[StaticRule] = &[
    StaticRule {
        code: "EULA_FILE",
        check: regla_eula_file,
    },
    StaticRule {
        code: "SYNC_CHUNK_WRITES",
        check: regla_sync_chunk_writes,
    },
    StaticRule {
        code: "RCON_PASSWORD",
        check: regla_rcon_password,
    },
    StaticRule {
        code: "JAVA_MAJOR",
        check: regla_java_major,
    },
    StaticRule {
        code: "PORT_BUSY",
        check: regla_port_busy,
    },
    StaticRule {
        code: "ENGINE_JAR_MISSING",
        check: regla_engine_jar_missing,
    },
    StaticRule {
        code: "PLUGIN_UPDATE",
        check: regla_plugin_update,
    },
];

/// El loader que el doctor usa para preguntar por plugins.
///
/// **No** es el `modrinthLoader` del plan: aquí el TS hace `engine === "folia" ?
/// "folia" : "paper"`, así que un server de fabric pregunta como si fuera paper. Es
/// un resto de cuando el doctor solo miraba plugins de Bukkit; se reproduce igual.
pub fn modrinth_loader_de(engine: EngineId) -> &'static str {
    if engine == EngineId::Folia {
        "folia"
    } else {
        "paper"
    }
}

/// Los slugs que el doctor reconoce en `plugins/` (`KNOWN_PLUGIN_SLUGS`).
pub const KNOWN_PLUGIN_SLUGS: &[&str] = &["chunky", "spark", "chunkyborder", "luckperms"];

/// El jar que arranca el server según `start.sh`/`start.bat`
/// (`jarNameFromStartScript`).
///
/// El `.bat` lo escribe como `set "JAR=x.jar"` y el `.sh` como `JAR="x.jar"`, así
/// que se prueban los dos patrones sobre el fichero que exista, en ese orden.
///
/// El `\r?` no es decoración: el `$` de JS en modo multilínea también casa antes de
/// un `\r`, y el de Rust no, así que sin él un `start.bat` con finales CRLF —que es
/// como se escribe un `.bat`— no daría el jar.
pub fn jar_name_from_start_script(v: &dyn VistaServidor) -> Option<String> {
    let re_bat = regex::Regex::new(r#"(?m)^set "JAR=(.+)"\r?$"#).expect("patrón constante");
    let re_sh = regex::Regex::new(r#"(?m)^JAR="(.+)"\r?$"#).expect("patrón constante");
    for f in ["start.sh", "start.bat"] {
        let Some(txt) = v.leer(f) else { continue };
        if let Some(c) = re_bat.captures(&txt).or_else(|| re_sh.captures(&txt)) {
            return Some(c[1].to_string());
        }
    }
    None
}

/// Evalúa las reglas estáticas (`evaluateStaticRules`).
///
/// Una regla que no encuentra nada devuelve `None` y no aparece en el informe; el
/// orden es el de `STATIC_RULES`, que es el que ve el usuario antes de las de log.
pub fn evaluar_estaticas(
    vista: &dyn VistaServidor,
    ctx: &ContextoEstatico,
    hechos: &dyn Hechos,
) -> Vec<Finding> {
    STATIC_RULES
        .iter()
        .filter_map(|r| (r.check)(vista, ctx, hechos))
        .collect()
}

/// Constructor de hallazgos con fix, para no repetir siete veces lo mismo.
fn hallazgo(
    severity: Severity,
    code: &str,
    source: &str,
    message: String,
    fix: Option<(&str, String, bool)>,
) -> Finding {
    Finding {
        severity,
        code: code.to_string(),
        message,
        source: source.to_string(),
        fix_code: fix.as_ref().map(|(c, _, _)| c.to_string()),
        fix_considerable: fix.as_ref().is_some_and(|(_, _, cons)| *cons),
        fix_description: fix.map(|(_, d, _)| d),
    }
}

fn regla_eula_file(
    v: &dyn VistaServidor,
    _c: &ContextoEstatico,
    _h: &dyn Hechos,
) -> Option<Finding> {
    // `includes("eula=true")`: es una subcadena, no una línea ni una igualdad
    if v.existe("eula.txt") && v.leer("eula.txt").unwrap_or_default().contains("eula=true") {
        return None;
    }
    Some(hallazgo(
        Severity::Error,
        "EULA_FILE",
        "eula.txt",
        "Falta eula=true: el server se apagaría al primer arranque.".to_string(),
        Some(("accept-eula", "Aceptar EULA (eula=true)".to_string(), false)),
    ))
}

fn regla_sync_chunk_writes(
    v: &dyn VistaServidor,
    _c: &ContextoEstatico,
    _h: &dyn Hechos,
) -> Option<Finding> {
    let txt = v.leer("server.properties")?;
    let re = regex::Regex::new("sync-chunk-writes=true").expect("patrón constante");
    if !re.is_match(&txt) {
        return None;
    }
    Some(hallazgo(
        Severity::Warning,
        "SYNC_CHUNK_WRITES",
        "server.properties",
        "sync-chunk-writes=true mata el rendimiento del IO (default de Mojang).".to_string(),
        Some((
            "sync-chunk-writes-off",
            "Poner sync-chunk-writes=false".to_string(),
            false,
        )),
    ))
}

fn regla_rcon_password(
    v: &dyn VistaServidor,
    _c: &ContextoEstatico,
    _h: &dyn Hechos,
) -> Option<Finding> {
    let txt = v.leer("server.properties")?;
    let activo = regex::Regex::new("enable-rcon=true").expect("patrón constante");
    if !activo.is_match(&txt) {
        return None;
    }
    let captura = regex::Regex::new(r"rcon.password=(.*)").expect("patrón constante");
    let pw = captura
        .captures(&txt)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    // `.length` de JS cuenta unidades UTF-16, no caracteres: un emoji son dos
    if pw.encode_utf16().count() >= 8 {
        return None;
    }
    Some(hallazgo(
        Severity::Error,
        "RCON_PASSWORD",
        "server.properties",
        "RCON habilitado con contraseña débil o vacía.".to_string(),
        Some((
            "rcon-password",
            "Generar contraseña RCON aleatoria".to_string(),
            false,
        )),
    ))
}

fn regla_java_major(
    v: &dyn VistaServidor,
    c: &ContextoEstatico,
    h: &dyn Hechos,
) -> Option<Finding> {
    let ruta = match v.leer("runtime/java.path") {
        Some(t) => t.trim().to_string(),
        None => "java".to_string(),
    };
    let mayor = h.java_mayor(&ruta);
    let requerido = crate::catalog::java_major_for(&c.mc_version);
    if mayor == Some(requerido) {
        return None;
    }
    // `major ?? "?"`
    let texto = match mayor {
        Some(m) => m.to_string(),
        None => "?".to_string(),
    };
    Some(hallazgo(
        Severity::Error,
        "JAVA_MAJOR",
        "runtime/java.path",
        format!(
            "Java {texto} en uso; {} requiere Java {requerido}.",
            c.mc_version
        ),
        Some((
            "install-java",
            format!("Instalar Temurin JDK {requerido} en runtime/"),
            false,
        )),
    ))
}

fn regla_port_busy(v: &dyn VistaServidor, c: &ContextoEstatico, h: &dyn Hechos) -> Option<Finding> {
    let _ = v;
    if h.puerto_libre(c.port) {
        return None;
    }
    Some(hallazgo(
        Severity::Error,
        "PORT_BUSY",
        &format!("puerto {}", c.port),
        format!("El puerto {} ya está ocupado (¿otro server?).", c.port),
        Some(("change-port", "Cambiar a un puerto libre".to_string(), true)),
    ))
}

fn regla_engine_jar_missing(
    v: &dyn VistaServidor,
    c: &ContextoEstatico,
    _h: &dyn Hechos,
) -> Option<Finding> {
    let jar = jar_name_from_start_script(v);
    if let Some(j) = &jar {
        if v.existe(j) {
            return None;
        }
    }
    if v.existe("run.sh") || v.existe("run.bat") {
        return None;
    }
    let entre = match &jar {
        Some(j) => format!(" ({j})"),
        None => String::new(),
    };
    Some(hallazgo(
        Severity::Error,
        "ENGINE_JAR_MISSING",
        &c.dir,
        format!("Falta el jar del engine{entre}: instala de nuevo."),
        // sin fix: no hay nada que este doctor pueda hacer por ti
        None,
    ))
}

fn regla_plugin_update(
    v: &dyn VistaServidor,
    c: &ContextoEstatico,
    h: &dyn Hechos,
) -> Option<Finding> {
    let instalados = v.listar("plugins")?;
    let loader = modrinth_loader_de(c.engine);
    for f in instalados {
        if !f.ends_with(".jar") {
            continue;
        }
        let minuscula = f.to_lowercase();
        let Some(slug) = KNOWN_PLUGIN_SLUGS
            .iter()
            .find(|s| minuscula.contains(&s.to_lowercase().replace('-', "")))
        else {
            continue;
        };
        if let Some(version) = h.ultima_version(slug, &c.mc_version, loader) {
            if let Some(fichero) = crate::modrinth::primary_file(&version) {
                if !fichero.filename.is_empty() && fichero.filename != f {
                    return Some(hallazgo(
                        Severity::Warning,
                        "PLUGIN_UPDATE",
                        &format!("plugins/{f}"),
                        format!("{f} tiene versión más nueva: {}.", fichero.filename),
                        Some((
                            "update-plugin",
                            format!("Actualizar a {}", fichero.filename),
                            false,
                        )),
                    ));
                }
            }
        }
        // solo el primero: el doctor itera en siguientes pasadas
        break;
    }
    None
}
