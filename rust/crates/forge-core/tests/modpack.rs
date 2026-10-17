//! Parsers de modpack (Modrinth `.mrpack` y CurseForge), contra el golden del TS.

use forge_core::modrinth;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn golden() -> serde_json::Value {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/modpack.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("golden inválido")
}

/// El JSON de entrada de cada caso, para no duplicarlo en el test.
fn json_del_caso(nombre: &str) -> String {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/modpack-entradas.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    let g: serde_json::Value = serde_json::from_str(&txt).expect("entradas inválidas");
    g[nombre]
        .as_str()
        .unwrap_or_else(|| panic!("sin entrada para «{nombre}»"))
        .to_string()
}

// ---------- .mrpack ----------

#[test]
fn el_mrpack_del_ts_parsea_y_filtra_igual() {
    // el caso que prueba `tests/protocol.test.ts:49-55`
    let idx = modrinth::parse_mrpack_index(&json_del_caso("mrpack-valido")).expect("parsea");
    assert_eq!(idx.format_version, 1);
    assert_eq!(
        idx.dependencies.get("minecraft").map(String::as_str),
        Some("1.21.1")
    );
    assert_eq!(
        idx.dependencies.get("fabric-loader").map(String::as_str),
        Some("0.16.9")
    );

    // el filtro del TS: `env?.server !== "unsupported"`, y la entrada SIN env se
    // incluye (es lo que mete `config/optimizado.toml` en el servidor)
    let servidor: Vec<&str> = modrinth::mrpack_server_files(&idx)
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(
        servidor,
        vec![
            "mods/lithium.jar",
            "config/optimizado.toml",
            "mods/sin-env.jar"
        ]
    );
}

#[test]
fn el_formato_del_mrpack_se_valida() {
    // `formatVersion !== 1` es error, igual que el TS
    assert!(matches!(
        modrinth::parse_mrpack_index(&json_del_caso("mrpack-formato-2")),
        Err(modrinth::Error::FormatoMrpack(2))
    ));
    // sin `formatVersion`, el TS compara `undefined !== 1` y lanza su mensaje;
    // aquí serde no puede deserializar y devuelve otro error. Los dos fallan, que
    // es lo que importa: un formato desconocido no se interpreta a medias.
    assert!(modrinth::parse_mrpack_index(&json_del_caso("mrpack-sin-formato")).is_err());
    assert!(modrinth::parse_mrpack_index("{ no es json }").is_err());
}

#[test]
fn un_mrpack_sin_ficheros_no_revienta() {
    // Divergencia deliberada, y es una mejora: en el TS `parseMrpackIndex` pasa y
    // `mrpackServerFiles` revienta con "Cannot read properties of undefined
    // (reading 'filter')", porque `idx.files` es undefined y no lo comprueba. Aquí
    // `files` tiene `#[serde(default)]`, así que el índice parsea y el filtro
    // devuelve una lista vacía. El puerto no replica los panics.
    let idx = modrinth::parse_mrpack_index(&json_del_caso("mrpack-sin-ficheros")).expect("parsea");
    assert!(modrinth::mrpack_server_files(&idx).is_empty());
}

// ---------- CurseForge ----------

#[test]
fn el_manifest_de_curseforge_da_el_mismo_engine_y_build() {
    let g = golden();
    for caso in g["curseforge"].as_array().expect("curseforge") {
        let nombre = caso["nombre"].as_str().unwrap();
        let m = modrinth::parse_cf_manifest(&json_del_caso(nombre)).expect("parsea");
        let k = modrinth::cf_loader_kind(&m);
        let engine_esperado = caso["engine"].as_str();
        let build_esperado = caso["build"].as_str();
        assert_eq!(
            k.as_ref().map(|(e, _)| *e),
            engine_esperado,
            "engine de «{nombre}»"
        );
        assert_eq!(
            k.as_ref().map(|(_, b)| b.as_str()),
            build_esperado,
            "build de «{nombre}»"
        );
        assert_eq!(
            m.minecraft.version,
            caso["mc"].as_str().unwrap(),
            "mc de «{nombre}»"
        );
        assert_eq!(
            m.files.len(),
            caso["ficheros"].as_u64().unwrap() as usize,
            "ficheros de «{nombre}»"
        );
    }
}

#[test]
fn un_manifest_que_no_es_de_modpack_se_rechaza() {
    let g = golden();
    let casos = g["noModpack"].as_array().expect("noModpack");
    assert_eq!(casos.len(), 2);
    for caso in casos {
        let nombre = caso["nombre"].as_str().unwrap();
        let r = modrinth::parse_cf_manifest(&json_del_caso(nombre));
        assert!(r.is_err(), "«{nombre}» debe rechazarse");
        assert!(!caso["ok"].as_bool().unwrap(), "el TS tampoco lo acepta");
        assert!(
            caso["error"].as_str().is_some(),
            "el TS da un mensaje propio"
        );
    }
}

#[test]
fn el_loader_primario_es_el_que_cuenta() {
    // "sin primario" tiene un loader con primary: false, así que no hay engine
    let m = modrinth::parse_cf_manifest(&json_del_caso("sin primario")).expect("parsea");
    assert_eq!(modrinth::cf_loader_kind(&m), None);
    // y "forge con build vacío" da build vacío, no None: el prefijo está
    let m2 = modrinth::parse_cf_manifest(&json_del_caso("forge con build vacío")).expect("parsea");
    assert_eq!(
        modrinth::cf_loader_kind(&m2),
        Some(("forge", String::new()))
    );
}
