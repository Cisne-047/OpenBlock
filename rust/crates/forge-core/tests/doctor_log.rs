//! Las reglas de log del doctor, contra el golden que genera el TypeScript.
//!
//! Son puras —texto y la parte de la config que leen las descripciones— y llevan el
//! detalle que un puerto se salta: **el primer match** de cada regla, no el último
//! ni la suma, y cada regla barre el log por su cuenta (una línea puede disparar
//! varias). Lo que no se compara es el `apply` de cada fix: eso escribe ficheros y
//! arranca procesos, y vive en el CLI.

use forge_core::doctor::{evaluar_log, Capturas, ContextoLog, Finding, Severity, LOG_RULES};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Caso {
    nombre: String,
    args: Vec<Value>,
    salida_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReglaGolden {
    code: String,
    severity: String,
    message: String,
    tiene_fix: bool,
}

#[derive(Debug, Deserialize)]
struct Entrada {
    lineas: Vec<String>,
    #[serde(rename = "viewDistance")]
    view_distance: u32,
}

/// La proyección del hallazgo que se compara. El orden de los campos es el del TS
/// (`JSON.stringify` escribe en orden de declaración) y `fix` va a `null` cuando no
/// hay, porque el TS escribe `undefined`… pero aquí se proyecta a mano en los dos
/// lados, así que `null` es lo acordado.
#[derive(Debug, Serialize)]
struct ProyFinding {
    severity: &'static str,
    code: String,
    source: String,
    message: String,
    fix: Option<ProyFix>,
}

#[derive(Debug, Serialize)]
struct ProyFix {
    code: String,
    description: String,
    considerable: bool,
}

fn proyeccion(f: &Finding) -> ProyFinding {
    ProyFinding {
        severity: f.severity.as_str(),
        code: f.code.clone(),
        source: f.source.clone(),
        message: f.message.clone(),
        fix: f.fix_code.as_ref().map(|code| ProyFix {
            code: code.clone(),
            description: f.fix_description.clone().unwrap_or_default(),
            considerable: f.fix_considerable,
        }),
    }
}

fn golden() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/doctor-log.json");
    let txt = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("el golden debe ser JSON")
}

#[test]
fn las_reglas_son_las_mismas_y_en_el_mismo_orden() {
    let g = golden();
    let reglas: Vec<ReglaGolden> = serde_json::from_value(g["reglas"].clone()).expect("reglas");
    assert!(!reglas.is_empty());
    assert_eq!(
        reglas.len(),
        LOG_RULES.len(),
        "el TS tiene {} reglas y Rust {}",
        reglas.len(),
        LOG_RULES.len()
    );
    for (i, esperada) in reglas.iter().enumerate() {
        let r = &LOG_RULES[i];
        assert_eq!(r.code, esperada.code, "el orden de las reglas cambió");
        assert_eq!(
            r.severity.as_str(),
            esperada.severity,
            "gravedad de {}",
            r.code
        );
        assert_eq!(r.message, esperada.message, "mensaje de {}", r.code);
        assert_eq!(r.fix.is_some(), esperada.tiene_fix, "fix de {}", r.code);
    }
}

#[test]
fn la_evaluacion_del_log_coincide_con_la_del_ts() {
    let g = golden();
    let casos: Vec<Caso> = serde_json::from_value(g["casos"].clone()).expect("casos");
    assert!(!casos.is_empty());
    for c in casos {
        let entrada: Entrada = serde_json::from_value(c.args[0].clone()).expect("entrada del caso");
        let ctx = ContextoLog {
            view_distance: entrada.view_distance,
        };
        let salida: Vec<ProyFinding> = evaluar_log(&entrada.lineas, &ctx)
            .iter()
            .map(proyeccion)
            .collect();
        let json = serde_json::to_string(&salida).expect("serializable");
        assert_eq!(
            json,
            c.salida_json.clone().unwrap_or_else(|| "null".to_string()),
            "caso «{}»",
            c.nombre
        );
    }
}

#[test]
fn un_grupo_que_no_casa_se_escribe_como_undefined() {
    // lo que haría el TS al meter un grupo ausente en una plantilla
    let vacias = Capturas(vec![None, None]);
    assert_eq!(vacias.grupo(1), "undefined");
    let con = Capturas(vec![Some("todo".to_string()), Some("21".to_string())]);
    assert_eq!(con.grupo(1), "21");
    // el grupo 0 es el match entero, como en JS
    assert_eq!(con.grupo(0), "todo");
}

#[test]
fn la_descripcion_de_la_view_distance_tiene_suelo() {
    // `Math.max(4, viewDistance - 2)`, incluida la resta saturante por debajo de 2
    let regla = LOG_RULES
        .iter()
        .find(|r| r.code == "TICK_BEHIND")
        .expect("la regla existe");
    let fix = regla.fix.as_ref().expect("tiene fix");
    let lineas = vec!["Can't keep up!".to_string()];
    for (vd, esperado) in [
        (12u32, "de 12 a 10"),
        (6, "de 6 a 4"),
        (5, "de 5 a 4"),
        (2, "de 2 a 4"),
        (0, "de 0 a 4"),
    ] {
        let ctx = ContextoLog { view_distance: vd };
        let f = evaluar_log(&lineas, &ctx);
        assert_eq!(f.len(), 1);
        let desc = f[0].fix_description.clone().unwrap();
        assert!(
            desc.contains(esperado),
            "con viewDistance {vd} esperaba «{esperado}» y salió «{desc}»"
        );
        let _ = fix;
    }
}

#[test]
fn sin_fix_no_hay_codigo_ni_descripcion() {
    let ctx = ContextoLog { view_distance: 8 };
    let hallazgos = evaluar_log(&["Unsupported API version 1.20".to_string()], &ctx);
    assert_eq!(hallazgos.len(), 1);
    assert_eq!(hallazgos[0].severity, Severity::Info);
    assert!(hallazgos[0].fix_code.is_none());
    assert!(hallazgos[0].fix_description.is_none());
    assert!(!hallazgos[0].fix_considerable);
}
