//! Engines: el mapeo de las respuestas de las APIs de descarga, contra el golden
//! que genera el TypeScript (`npm run parity:golden`).
//!
//! La respuesta de una API ajena no se puede comparar —cambia sin avisar—, pero
//! el mapeo sí, y ahí están todas las rarezas: el `reduce` que no es el máximo,
//! el `??` por orden de escritura, el `Set` que deduplica antes de invertir y los
//! `String(undefined)` que acaban dentro de una URL.
//!
//! La comparación es contra `salida_json`, la serialización que hizo el TS, no
//! contra el valor suelto: así se comprueba también **el orden de las claves**,
//! que es lo que decide si un plan se puede reimportar.

use forge_api::engines::*;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Caso {
    nombre: String,
    args: Vec<Value>,
    salida_json: Option<String>,
    error: Option<String>,
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn golden() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/engines.json");
    let txt = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("el golden debe ser JSON")
}

/// Los casos de una sección, con el nombre de la sección en el mensaje de fallo.
fn casos(g: &Value, seccion: &str) -> Vec<Caso> {
    let lista = g.get(seccion).unwrap_or_else(|| {
        panic!(
            "el golden no tiene la sección «{seccion}» (¿lo has regenerado? en {})",
            raiz().display()
        )
    });
    serde_json::from_value(lista.clone())
        .unwrap_or_else(|e| panic!("sección «{seccion}» ilegible: {e}"))
}

fn texto(c: &Caso, i: usize) -> String {
    c.args[i]
        .as_str()
        .unwrap_or_else(|| panic!("el argumento {i} de «{}» debe ser texto", c.nombre))
        .to_string()
}

/// Un argumento opcional: en el golden, `null` es «no se pasa».
fn opcional(c: &Caso, i: usize) -> Option<String> {
    c.args[i].as_str().map(str::to_string)
}

/// Comprueba un caso: si el TS lanzó, Rust tiene que fallar con **ese** mensaje;
/// si devolvió algo, la serialización tiene que ser idéntica byte a byte.
fn revisa<T: Serialize>(seccion: &str, c: &Caso, salida: Result<T, Error>) {
    match (&c.error, salida) {
        (Some(esperado), Err(e)) => {
            assert_eq!(
                &e.to_string(),
                esperado,
                "{seccion} / «{}»: el mensaje de error no coincide",
                c.nombre
            )
        }
        (Some(esperado), Ok(v)) => panic!(
            "{seccion} / «{}»: el TS lanza {esperado:?} y Rust devolvió {}",
            c.nombre,
            serde_json::to_string(&v).unwrap()
        ),
        (None, Ok(v)) => {
            let json = serde_json::to_string(&v).expect("serializable");
            let esperado = c.salida_json.clone().unwrap_or_else(|| "null".to_string());
            assert_eq!(
                json, esperado,
                "{seccion} / «{}»: la salida no coincide",
                c.nombre
            );
        }
        (None, Err(e)) => panic!(
            "{seccion} / «{}»: el TS devuelve {} y Rust falla: {e}",
            c.nombre,
            c.salida_json.clone().unwrap_or_else(|| "null".to_string())
        ),
    }
}

/// Envuelve una salida directa (las funciones puras que no devuelven `Result`).
fn ok<T>(v: T) -> Result<T, Error> {
    Ok(v)
}

#[test]
fn encode_uri_component_como_el_de_js() {
    let g = golden();
    let lista = g
        .get("encode_uri_component")
        .expect("sección encode_uri_component");
    let casos: Vec<Value> = serde_json::from_value(lista.clone()).unwrap();
    assert!(!casos.is_empty());
    for c in casos {
        let entrada = c["entrada"].as_str().unwrap();
        assert_eq!(
            forge_api::url::encode_uri_component(entrada),
            c["salida"].as_str().unwrap(),
            "encodeURIComponent({entrada:?})"
        );
    }
}

#[test]
fn paper_builds_ordena_descendente_por_numero() {
    let g = golden();
    for c in casos(&g, "paper_builds_from") {
        revisa("paper_builds_from", &c, ok(paper_builds_from(&c.args[0])));
    }
}

#[test]
fn papermc_v3_reproduce_el_reduce_y_el_orden_de_las_descargas() {
    let g = golden();
    for c in casos(&g, "papermc_v3_from") {
        let salida = papermc_v3_from(&c.args[0], &texto(&c, 1), &texto(&c, 2));
        revisa("papermc_v3_from", &c, ok(salida));
    }
}

#[test]
fn papermc_v2_busca_la_build_y_si_no_esta_lo_dice() {
    let g = golden();
    for c in casos(&g, "papermc_v2_from") {
        let build = opcional(&c, 3);
        let salida = papermc_v2_from(&c.args[0], &texto(&c, 1), &texto(&c, 2), build.as_deref());
        revisa("papermc_v2_from", &c, salida);
    }
}

#[test]
fn purpur_invierte_la_lista_de_builds() {
    let g = golden();
    for c in casos(&g, "purpur_builds_from") {
        revisa("purpur_builds_from", &c, purpur_builds_from(&c.args[0]));
    }
}

#[test]
fn purpur_construye_la_url_de_descarga() {
    let g = golden();
    for c in casos(&g, "purpur_download_from") {
        let build = opcional(&c, 2);
        let salida = purpur_download_from(&c.args[0], &texto(&c, 1), build.as_deref());
        revisa("purpur_download_from", &c, salida);
    }
}

#[test]
fn latest_version_se_queda_con_la_primera_sin_stable_false() {
    let g = golden();
    for c in casos(&g, "latest_version_from") {
        let salida = latest_version_from(&c.args[0], &texto(&c, 1), &texto(&c, 2));
        revisa("latest_version_from", &c, salida);
    }
}

#[test]
fn las_versiones_de_loader_de_fabric() {
    let g = golden();
    for c in casos(&g, "fabric_loader_versions_from") {
        revisa(
            "fabric_loader_versions_from",
            &c,
            fabric_loader_versions_from(&c.args[0]),
        );
    }
}

#[test]
fn los_installers_de_fabric_no_pasan_por_string() {
    let g = golden();
    for c in casos(&g, "fabric_installer_versions_from") {
        revisa(
            "fabric_installer_versions_from",
            &c,
            fabric_installer_versions_from(&c.args[0]),
        );
    }
}

#[test]
fn fabric_no_escapa_el_loader_en_la_url() {
    let g = golden();
    for c in casos(&g, "fabric_download_from") {
        let salida = fabric_download_from(&texto(&c, 0), &texto(&c, 1), &texto(&c, 2));
        revisa("fabric_download_from", &c, ok(salida));
    }
}

#[test]
fn las_versiones_de_quilt() {
    let g = golden();
    for c in casos(&g, "quilt_loader_versions_from") {
        revisa(
            "quilt_loader_versions_from",
            &c,
            quilt_loader_versions_from(&c.args[0]),
        );
    }
    for c in casos(&g, "quilt_installer_versions_from") {
        revisa(
            "quilt_installer_versions_from",
            &c,
            quilt_installer_versions_from(&c.args[0]),
        );
    }
    for c in casos(&g, "quilt_download_from") {
        let salida = quilt_download_from(&texto(&c, 0), &texto(&c, 1), &texto(&c, 2));
        revisa("quilt_download_from", &c, ok(salida));
    }
}

#[test]
fn forge_deduplica_y_luego_invierte() {
    let g = golden();
    for c in casos(&g, "forge_builds_from") {
        let salida = forge_builds_from(&texto(&c, 0), &texto(&c, 1));
        revisa("forge_builds_from", &c, salida);
    }
}

#[test]
fn forge_elige_recommended_y_si_no_latest() {
    let g = golden();
    for c in casos(&g, "forge_pick_from") {
        let salida = forge_pick_from(&c.args[0], &texto(&c, 1));
        revisa("forge_pick_from", &c, salida);
    }
    for c in casos(&g, "forge_download_from") {
        let salida = forge_download_from(&texto(&c, 0), &texto(&c, 1));
        revisa("forge_download_from", &c, ok(salida));
    }
}

#[test]
fn el_prefijo_de_neoforge_pierde_el_primer_segmento() {
    let g = golden();
    let lista = g.get("neoforge_prefix").expect("sección neoforge_prefix");
    let casos: Vec<Value> = serde_json::from_value(lista.clone()).unwrap();
    assert!(!casos.is_empty());
    for c in casos {
        assert_eq!(
            neoforge_prefix(c["entrada"].as_str().unwrap()),
            c["salida"].as_str().unwrap(),
            "neoforgePrefix({:?})",
            c["entrada"]
        );
    }
}

#[test]
fn neoforge_filtra_por_prefijo_y_coge_la_ultima() {
    let g = golden();
    for c in casos(&g, "neoforge_versions_from") {
        let salida = neoforge_versions_from(&c.args[0], &texto(&c, 1));
        revisa("neoforge_versions_from", &c, salida);
    }
    for c in casos(&g, "neoforge_builds_from") {
        let salida = neoforge_builds_from(&c.args[0], &texto(&c, 1));
        revisa("neoforge_builds_from", &c, salida);
    }
    for c in casos(&g, "neoforge_download_from") {
        let salida = neoforge_download_from(&texto(&c, 0), &texto(&c, 1));
        revisa("neoforge_download_from", &c, ok(salida));
    }
}
