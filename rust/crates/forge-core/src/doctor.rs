//! La parte pura del doctor, portada de `src/doctor.ts`.
//!
//! El doctor completo necesita red, disco, procesos y RCON, así que va al CLI. Pero
//! su **aritmética** —el score, las notas, el dedupe y la etiqueta de pendiente— es
//! pura, y es justo donde un puerto se equivoca en silencio: un score mal calculado
//! no da error, da un informe que miente sobre el estado del server.
//!
//! Las reglas como datos (`LOG_RULES`, `STATIC_RULES`) y la ejecución de fixes se
//! quedan para el CLI, que es donde están los efectos.

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
