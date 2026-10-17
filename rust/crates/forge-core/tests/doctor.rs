//! La parte pura del doctor: score, nota, dedupe y etiquetas, contra el golden del TS.
//!
//! Es la aritmética del informe, y por eso importa: un score mal calculado no da
//! error, da un informe que miente sobre el estado del server.

use forge_core::doctor::{self, Finding, Severity};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn golden() -> serde_json::Value {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/doctor.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("golden inválido")
}

/// Construye tantos hallazgos como digan las cuentas, con códigos distintos.
fn hallazgos(errors: usize, warnings: usize, infos: usize) -> Vec<Finding> {
    let mut v = Vec::new();
    for i in 0..errors {
        v.push(Finding::new(Severity::Error, &format!("E{i}")));
    }
    for i in 0..warnings {
        v.push(Finding::new(Severity::Warning, &format!("W{i}")));
    }
    for i in 0..infos {
        v.push(Finding::new(Severity::Info, &format!("I{i}")));
    }
    v
}

#[test]
fn el_score_y_la_nota_coinciden_con_el_ts() {
    let g = golden();
    let casos = g["scores"].as_array().expect("scores");
    assert!(
        casos.len() > 40,
        "el golden debería cubrir muchas combinaciones"
    );

    for caso in casos {
        let errors = caso["errors"].as_u64().unwrap() as usize;
        let warnings = caso["warnings"].as_u64().unwrap() as usize;
        let infos = caso["infos"].as_u64().unwrap() as usize;
        let score_esperado = caso["score"].as_u64().unwrap() as u32;
        let nota_esperada = caso["grade"].as_str().unwrap();

        let (score, nota) = doctor::score_y_nota(&hallazgos(errors, warnings, infos));
        assert_eq!(
            score, score_esperado,
            "score con {errors} errores, {warnings} warnings y {infos} infos"
        );
        assert_eq!(nota, nota_esperada, "nota del score {score}");
    }
}

#[test]
fn info_no_resta_al_score() {
    // es la trampa de este cálculo: `info` se cuenta para el informe, no para la
    // nota. Tratarlo como warning bajaría 5 puntos por algo que no está mal.
    let (sin_info, _) = doctor::score_y_nota(&hallazgos(1, 1, 0));
    let (con_info, _) = doctor::score_y_nota(&hallazgos(1, 1, 10));
    assert_eq!(sin_info, con_info, "los info no pueden mover el score");
    assert_eq!(sin_info, 75, "1 error (-20) + 1 warning (-5) = 75");
}

#[test]
fn el_score_tiene_suelo_cero_y_no_techo_negativo() {
    // con 5 errores ya se llega a 0, y con 50 no puede quedarse en negativo
    assert_eq!(doctor::score_y_nota(&hallazgos(5, 0, 0)).0, 0);
    assert_eq!(doctor::score_y_nota(&hallazgos(50, 50, 50)).0, 0);
    assert_eq!(doctor::score_y_nota(&[]).0, 100);
}

#[test]
fn los_cortes_de_nota_estan_donde_dice_el_ts() {
    assert_eq!(doctor::nota(100), "SANO");
    assert_eq!(doctor::nota(90), "SANO");
    assert_eq!(doctor::nota(89), "OK CON AVISOS");
    assert_eq!(doctor::nota(70), "OK CON AVISOS");
    assert_eq!(doctor::nota(69), "NECESITA ATENCIÓN");
    assert_eq!(doctor::nota(40), "NECESITA ATENCIÓN");
    assert_eq!(doctor::nota(39), "CRÍTICO");
    assert_eq!(doctor::nota(0), "CRÍTICO");
}

#[test]
fn el_dedupe_se_queda_con_el_primero_y_conserva_el_orden() {
    let g = golden();
    let entrada = g["dedupe"]["entrada"].as_array().expect("entrada");
    let esperado = g["dedupe"]["salida"].as_array().expect("salida");

    let findings: Vec<Finding> = entrada
        .iter()
        .map(|f| {
            Finding::new(
                Severity::parse(f["severity"].as_str().unwrap()).unwrap(),
                f["code"].as_str().unwrap(),
            )
        })
        .collect();

    let unicos = doctor::dedupe_por_codigo(&findings);
    let codigos: Vec<&str> = unicos.iter().map(|f| f.code.as_str()).collect();
    let esperados: Vec<&str> = esperado
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    assert_eq!(codigos, esperados);
    // y la gravedad que sobrevive es la del primero, no la del último
    assert_eq!(unicos[0].severity, Severity::Error);
}

#[test]
fn la_etiqueta_de_pendiente_marca_los_automaticos() {
    let g = golden();
    for caso in g["etiquetas"].as_array().expect("etiquetas") {
        let descripcion = caso["descripcion"].as_str().unwrap();
        let considerable = caso["considerable"].as_bool().unwrap();
        let esperada = caso["etiqueta"].as_str().unwrap();
        assert_eq!(
            doctor::etiqueta_pendiente(descripcion, considerable),
            esperada,
            "etiqueta de «{descripcion}»"
        );
    }
}

#[test]
fn un_hallazgo_sin_fix_sigue_restando_aunque_se_arregle_todo_lo_demas() {
    // Es el caso del self-test del TS: con el EULA arreglado y el jar ausente —que
    // no tiene fix— el score queda en 80. Un hallazgo sin fix nunca se filtra.
    let findings = vec![
        Finding::con_fix(Severity::Error, "EULA_FILE", "accept-eula", false),
        Finding::new(Severity::Error, "ENGINE_JAR_MISSING"),
    ];
    let aplicados = vec!["accept-eula".to_string()];
    let rotos = doctor::siguen_rotos(&findings, &aplicados);
    assert_eq!(rotos.len(), 1);
    assert_eq!(rotos[0].code, "ENGINE_JAR_MISSING");
    // -20 por el jar ausente: 80, y OK CON AVISOS
    let (score, nota) = doctor::score_y_nota(&rotos);
    assert_eq!((score, nota), (80, "OK CON AVISOS"));
}

#[test]
fn aplicar_un_fix_que_comparten_dos_reglas_las_arregla_las_dos() {
    // `JAVA_VERSION` y `JAVA_MAJOR` producen las dos `install-java`: el filtro va
    // por código de fix, no por código de regla
    let findings = vec![
        Finding::con_fix(Severity::Error, "JAVA_VERSION", "install-java", false),
        Finding::con_fix(Severity::Error, "JAVA_MAJOR", "install-java", false),
    ];
    let rotos = doctor::siguen_rotos(&findings, &["install-java".to_string()]);
    assert!(
        rotos.is_empty(),
        "una sola aplicación arregla las dos reglas"
    );
}
