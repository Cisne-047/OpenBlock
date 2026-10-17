//! Cliente de Modrinth: la query, la primera versión y el fichero primario,
//! contra el golden que genera el TypeScript.
//!
//! Lo que se compara aquí es lo que el `plan` necesita y no puede depender de la
//! API: cómo se escribe la query, con quién se queda de la lista y qué fichero
//! elige. La respuesta real de Modrinth no se prueba, cambia sin avisar.

use forge_api::modrinth::{self, ModrinthVersion};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Caso {
    nombre: String,
    args: Vec<Value>,
    salida_json: Option<String>,
    error: Option<String>,
}

fn golden() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modrinth.json");
    let txt = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("el golden debe ser JSON")
}

fn casos(g: &Value, seccion: &str) -> Vec<Caso> {
    let lista = g.get(seccion).unwrap_or_else(|| {
        panic!("el golden no tiene la sección «{seccion}» (¿lo has regenerado?)")
    });
    serde_json::from_value(lista.clone())
        .unwrap_or_else(|e| panic!("sección «{seccion}» ilegible: {e}"))
}

fn revisa<T: Serialize>(seccion: &str, c: &Caso, salida: Result<T, modrinth::Error>) {
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

/// Una lista de strings del golden: ausente es `None`, vacía es `Some(vec![])`.
fn lista(c: &Caso, clave: &str) -> Option<Vec<String>> {
    match c.args[0].get(clave) {
        None | Some(Value::Null) => None,
        Some(v) => Some(serde_json::from_value(v.clone()).expect("lista de strings")),
    }
}

#[test]
fn la_query_de_modrinth_no_es_encodeuricomponent() {
    let g = golden();
    for c in casos(&g, "latest_version_params") {
        let game_versions = lista(&c, "gameVersions");
        let loaders = lista(&c, "loaders");
        let salida = modrinth::latest_version_params(game_versions.as_deref(), loaders.as_deref());
        revisa("latest_version_params", &c, Ok(salida));
    }
}

#[test]
fn se_queda_con_la_primera_version_de_la_lista() {
    let g = golden();
    for c in casos(&g, "modrinth_version_from") {
        let slug = c.args[1].as_str().expect("el slug va como texto");
        let salida = modrinth::modrinth_version_from(&c.args[0], slug);
        revisa("modrinth_version_from", &c, salida);
    }
}

#[test]
fn el_fichero_primario_es_el_marcado_o_el_primero() {
    let g = golden();
    for c in casos(&g, "primary_file") {
        let v: ModrinthVersion =
            serde_json::from_value(c.args[0].clone()).expect("una versión válida");
        // `primaryFile` devuelve `undefined` sin ficheros: en JSON, `null`
        let salida: Option<&modrinth::ModrinthFile> = modrinth::primary_file(&v);
        revisa("primary_file", &c, Ok(salida));
    }
}

#[test]
fn el_slug_de_una_url_de_modrinth() {
    let g = golden();
    let lista = g
        .get("modrinth_modpack_slug")
        .expect("sección modrinth_modpack_slug");
    let casos: Vec<Value> = serde_json::from_value(lista.clone()).unwrap();
    assert!(!casos.is_empty());
    for c in casos {
        let entrada = c["entrada"].as_str().unwrap();
        assert_eq!(
            modrinth::modrinth_modpack_slug(entrada),
            c["salida"].as_str().unwrap(),
            "modrinthModpackSlug({entrada:?})"
        );
    }
}
