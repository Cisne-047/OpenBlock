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
