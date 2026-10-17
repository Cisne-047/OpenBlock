//! Mojang: la parte pura —parsear el manifiesto y filtrar releases— contra un
//! fixture. Lo que no se prueba es la respuesta real de la API, que no está bajo
//! nuestro control: eso es lo que el TS llama directamente y aquí sería un test
//! frágil.

use forge_api::mojang::{self, Manifest};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn manifiesto() -> Manifest {
    let p = repo_root().join("tests/fixtures/mojang-manifest.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("el fixture debe ser un manifiesto válido")
}

#[test]
fn parsea_el_manifiesto_de_mojang() {
    let m = manifiesto();
    assert_eq!(m.latest.release, "1.21.4");
    assert_eq!(m.latest.snapshot, "26w14a");
    assert_eq!(m.versions.len(), 8);
    // el `type` del JSON se llama `tipo` en Rust: serde lo mapea por rename
    assert_eq!(m.versions[0].tipo, "snapshot");
    assert_eq!(m.versions[1].id, "1.21.4");
    assert_eq!(m.versions[1].release_time, "2026-09-20T10:00:00+00:00");
}

#[test]
fn el_filtro_de_releases_salta_snapshots_y_betas() {
    // `listReleases` del TS: type === "release" y slice(0, limit)
    let m = manifiesto();
    assert_eq!(
        mojang::filtrar_releases(&m, 10),
        vec!["1.21.4", "1.21.3", "1.20.4", "1.19.4", "1.16.5"],
        "solo releases estables, en el orden del manifiesto"
    );
    // el límite corta
    assert_eq!(mojang::filtrar_releases(&m, 2), vec!["1.21.4", "1.21.3"]);
    assert!(mojang::filtrar_releases(&m, 0).is_empty());
}

#[test]
fn encuentra_versiones_por_id() {
    let m = manifiesto();
    let v = mojang::version_por_id(&m, "1.20.4").expect("1.20.4 está en el fixture");
    assert_eq!(
        v.url,
        "https://piston-meta.mojang.com/v1/packages/ddd/1.20.4.json"
    );
    // los tipos raros también se encuentran: el filtro solo aplica a listReleases
    assert!(mojang::version_por_id(&m, "26w14a").is_some());
    assert!(mojang::version_por_id(&m, "b1.7.3").is_some());
    assert!(mojang::version_por_id(&m, "9.9.9").is_none());
}

#[test]
fn un_manifiesto_sin_versiones_no_revienta() {
    // un JSON bien formado pero vacío: el filtro devuelve nada, no un panic
    let vacio: Manifest =
        serde_json::from_str(r#"{"latest":{"release":"x","snapshot":"y"},"versions":[]}"#).unwrap();
    assert!(mojang::filtrar_releases(&vacio, 10).is_empty());
    assert!(mojang::version_por_id(&vacio, "1.21.4").is_none());
}

#[test]
fn un_manifiesto_roto_es_un_error_no_un_panic() {
    // en el TS `JSON.parse` lanzaría; aquí `serde` devuelve Err y quien llama
    // decide. Es la misma pauta que en el resto del port.
    let r: Result<Manifest, _> = serde_json::from_str("{ esto no es json }");
    assert!(r.is_err());
}
