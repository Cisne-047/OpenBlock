//! Catálogo: mismos casos que `tests/catalog.test.ts`, más la paridad de las
//! tablas enteras contra el golden que genera el TS.

use forge_core::catalog;
use forge_core::LoaderKind;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

// ---------- parseMc (tests/catalog.test.ts:4-10) ----------

#[test]
fn parse_mc_con_y_sin_parche() {
    assert_eq!(catalog::parse_mc("1.21.4"), [1, 21, 4]);
    assert_eq!(catalog::parse_mc("1.20"), [1, 20, 0]);
    assert_eq!(catalog::parse_mc("26w14a"), [0, 0, 0]);
}

#[test]
fn parse_mc_bordes_del_regex() {
    // El regex retrocede: mayor y menor toman solo los dígitos iniciales de su
    // parte. Yo había supuesto [0,0,0] y el golden del TS dijo [1,2,0]: la
    // paridad cazó la suposición, que es exactamente para lo que está.
    assert_eq!(catalog::parse_mc("1.2b.3"), [1, 2, 0]);
    assert_eq!(catalog::parse_mc("1.20.b"), [1, 20, 0]);
    // sin punto tras el primer grupo no hay match: el snapshot devuelve ceros
    assert_eq!(catalog::parse_mc("26w14a"), [0, 0, 0]);
    assert_eq!(catalog::parse_mc("basura"), [0, 0, 0]);
    assert_eq!(catalog::parse_mc(""), [0, 0, 0]);
    // el parche no está anclado
    assert_eq!(catalog::parse_mc("1.20.4-rc1"), [1, 20, 4]);
    assert_eq!(catalog::parse_mc("26.3"), [26, 3, 0]);
}

// ---------- javaMajorFor (tests/catalog.test.ts:12-25) ----------

#[test]
fn mapa_de_java_por_mc() {
    assert_eq!(catalog::java_major_for("26.3"), 25);
    assert_eq!(catalog::java_major_for("26.1"), 25);
    assert_eq!(catalog::java_major_for("1.21.4"), 21);
    assert_eq!(catalog::java_major_for("1.20.6"), 21);
    assert_eq!(catalog::java_major_for("1.20.5"), 21);
    assert_eq!(catalog::java_major_for("1.20.4"), 17);
    assert_eq!(catalog::java_major_for("1.18.2"), 17);
    assert_eq!(catalog::java_major_for("1.17.1"), 17);
    assert_eq!(catalog::java_major_for("1.16.5"), 8);
    assert_eq!(catalog::java_major_for("1.12.2"), 8);
}

// ---------- versionKey (tests/catalog.test.ts:27-32) ----------

#[test]
fn version_key_ordena() {
    assert!(catalog::version_key([1, 21, 4]) > catalog::version_key([1, 20, 5]));
    assert!(catalog::version_key([1, 20, 5]) > catalog::version_key([1, 20, 4]));
}

// ---------- perfItemsFor (tests/catalog.test.ts:34-67) ----------

fn ids(loader: LoaderKind, mc: &str) -> Vec<&'static str> {
    catalog::perf_items_for(loader, mc)
        .iter()
        .map(|i| i.id)
        .collect()
}

#[test]
fn paper_recibe_plugins_no_mods_de_fabric() {
    let ids = ids(LoaderKind::Paper, "1.21.4");
    assert!(ids.contains(&"spark"));
    assert!(ids.contains(&"chunky"));
    assert!(!ids.contains(&"lithium"));
    assert!(!ids.contains(&"c2me"));
}

#[test]
fn fabric_recibe_lithium_c2me_krypton() {
    let ids = ids(LoaderKind::Fabric, "1.21.4");
    assert!(ids.contains(&"lithium"));
    assert!(ids.contains(&"c2me"));
    assert!(ids.contains(&"krypton"));
    assert!(!ids.contains(&"canary"));
}

#[test]
fn lazydfu_solo_hasta_1_19_4() {
    assert!(ids(LoaderKind::Fabric, "1.19.4").contains(&"lazydfu"));
    assert!(!ids(LoaderKind::Fabric, "1.20.4").contains(&"lazydfu"));
}

#[test]
fn canary_acotado_a_1_18_2_1_20_1() {
    assert!(ids(LoaderKind::Forge, "1.20.1").contains(&"canary"));
    assert!(!ids(LoaderKind::Forge, "1.21.4").contains(&"canary"));
    assert!(ids(LoaderKind::Forge, "1.18.2").contains(&"canary"));
    assert!(!ids(LoaderKind::Forge, "1.16.5").contains(&"canary"));
}

#[test]
fn vanilla_no_recibe_nada() {
    assert!(catalog::perf_items_for(LoaderKind::None, "1.21.4").is_empty());
}

// ---------- engineById (tests/catalog.test.ts:69-74) ----------

#[test]
fn folia_es_multithread_paper_no() {
    assert!(catalog::engine_by_id(forge_core::EngineId::Folia).multithread);
    assert!(!catalog::engine_by_id(forge_core::EngineId::Paper).multithread);
    assert!(catalog::engine_by_id(forge_core::EngineId::Forge).installer);
}

// ---------- paridad de las tablas enteras ----------

/// El golden lo genera el TS. Aquí se comprueba campo a campo: una errata en una
/// `desc` o un `reason` no la ve ningún test de comportamiento, y en cambio se
/// imprime en la CLI y en el README del proyecto generado.
#[test]
fn tablas_del_catalogo_coinciden_con_el_ts() {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/catalog.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    let g: serde_json::Value = serde_json::from_str(&txt).expect("golden inválido");

    // --- engines ---
    let engines = g["engines"].as_array().expect("engines");
    assert_eq!(engines.len(), catalog::ENGINES.len(), "número de engines");
    for (e, ge) in catalog::ENGINES.iter().zip(engines) {
        let id = ge["id"].as_str().unwrap();
        assert_eq!(e.id.as_str(), id, "id de engine");
        assert_eq!(e.label, ge["label"].as_str().unwrap(), "label de {id}");
        assert_eq!(
            e.loader.as_str(),
            ge["loader"].as_str().unwrap(),
            "loader de {id}"
        );
        assert_eq!(e.desc, ge["desc"].as_str().unwrap(), "desc de {id}");
        assert_eq!(
            e.plugin_api,
            ge["pluginApi"].as_bool().unwrap(),
            "pluginApi de {id}"
        );
        assert_eq!(e.mod_api, ge["modApi"].as_bool().unwrap(), "modApi de {id}");
        assert_eq!(
            e.multithread,
            ge["multithread"].as_bool().unwrap(),
            "multithread de {id}"
        );
        assert_eq!(
            e.installer,
            ge["installer"].as_bool().unwrap(),
            "installer de {id}"
        );
        assert_eq!(
            e.modpack_loader,
            ge["modpackLoader"].as_bool().unwrap(),
            "modpackLoader de {id}"
        );
    }

    // --- catálogo de rendimiento ---
    let items = g["perfItems"].as_array().expect("perfItems");
    assert_eq!(items.len(), catalog::PERF_ITEMS.len(), "número de items");
    for (it, gi) in catalog::PERF_ITEMS.iter().zip(items) {
        let id = gi["id"].as_str().unwrap();
        assert_eq!(it.id, id, "id");
        assert_eq!(it.label, gi["label"].as_str().unwrap(), "label de {id}");
        assert_eq!(it.kind, gi["kind"].as_str().unwrap(), "kind de {id}");
        assert_eq!(it.min_mc, gi["minMc"].as_str().unwrap(), "minMc de {id}");
        assert_eq!(it.max_mc, gi["maxMc"].as_str().unwrap(), "maxMc de {id}");
        assert_eq!(it.slug, gi["slug"].as_str().unwrap(), "slug de {id}");
        assert_eq!(it.reason, gi["reason"].as_str().unwrap(), "reason de {id}");
        assert_eq!(
            it.default,
            gi["default"].as_bool().unwrap(),
            "default de {id}"
        );
        let loaders: Vec<&str> = gi["loaders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mios: Vec<&str> = it.loaders.iter().map(|l| l.as_str()).collect();
        assert_eq!(mios, loaders, "loaders de {id}");
    }

    // --- resolución por loader × versión ---
    for (loader, casos) in g["perfPorLoader"].as_object().expect("perfPorLoader") {
        let lk =
            LoaderKind::parse(loader).unwrap_or_else(|| panic!("loader desconocido: {loader}"));
        for caso in casos.as_array().unwrap() {
            let mc = caso["mc"].as_str().unwrap();
            let esperado: Vec<&str> = caso["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(ids(lk, mc), esperado, "items de {loader} en {mc}");
        }
    }

    // --- mapa Java/MC, incluidos los bordes ---
    for (mc, esperado) in g["javaPorMc"].as_object().expect("javaPorMc") {
        assert_eq!(
            catalog::java_major_for(mc),
            esperado.as_u64().unwrap() as u32,
            "java de {mc}"
        );
    }

    // --- parseMc, incluidos los bordes del regex ---
    for (v, esperado) in g["parseMc"].as_object().expect("parseMc") {
        let arr = esperado.as_array().unwrap();
        let e: Vec<u32> = arr.iter().map(|x| x.as_u64().unwrap() as u32).collect();
        assert_eq!(catalog::parse_mc(v).to_vec(), e, "parseMc({v})");
    }
}
