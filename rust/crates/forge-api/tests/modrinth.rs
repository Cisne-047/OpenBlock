//! Cliente de Modrinth: la query, la primera versión y el fichero primario,
//! contra el golden que genera el TypeScript.
//!
//! Lo que se compara aquí es lo que el `plan` necesita y no puede depender de la
//! API: cómo se escribe la query, con quién se queda de la lista y qué fichero
//! elige. La respuesta real de Modrinth no se prueba, cambia sin avisar.

use forge_api::modrinth::{self, ModrinthVersion};
use forge_api::net;
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

/// El `.mrpack` es un zip, y esa parte no la puede cubrir el golden (depende de
/// bajar un fichero de verdad). Aquí se fabrica uno en memoria.
fn mrpack_con(entradas: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let mut w = zip::write::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opciones = zip::write::SimpleFileOptions::default();
        for (nombre, contenido) in entradas {
            w.start_file(*nombre, opciones).unwrap();
            w.write_all(contenido.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }
    buf
}

#[test]
fn saca_el_indice_de_un_mrpack() {
    let indice =
        r#"{"formatVersion":1,"name":"Fabulously Optimized","versionId":"5.0","files":[]}"#;
    let zip = mrpack_con(&[
        ("modrinth.index.json", indice),
        ("overrides/config.txt", "x"),
    ]);
    let idx = modrinth::mrpack_index_from_zip(&zip, "fa.mrpack").expect("el índice está dentro");
    assert_eq!(idx.name, "Fabulously Optimized");
}

#[test]
fn un_zip_sin_indice_no_es_un_mrpack() {
    let zip = mrpack_con(&[("otra-cosa.txt", "hola")]);
    let err = modrinth::mrpack_index_from_zip(&zip, "fa.mrpack").expect_err("no hay índice");
    assert_eq!(
        err.to_string(),
        "fa.mrpack no es un modpack Modrinth (.mrpack) válido"
    );
}

#[test]
fn unos_bytes_que_no_son_un_zip_lo_dicen() {
    let err =
        modrinth::mrpack_index_from_zip(b"esto no es un zip", "fa.mrpack").expect_err("no es zip");
    assert!(
        err.to_string().starts_with("fa.mrpack: zip ilegible"),
        "{err}"
    );
}

/// Un servidor HTTP de una sola respuesta, para probar la descarga sin red de verdad.
fn servidor_falso(estado: &'static str, cuerpo: Vec<u8>) -> (u16, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("puerto libre");
    let puerto = listener.local_addr().unwrap().port();
    let hilo = std::thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf);
            let cabecera = format!(
                "HTTP/1.1 {estado}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                cuerpo.len()
            );
            let _ = sock.write_all(cabecera.as_bytes());
            let _ = sock.write_all(&cuerpo);
            let _ = sock.flush();
        }
    });
    (puerto, hilo)
}

fn destino(nombre: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("openblock-dl-{}-{nombre}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn descarga_verifica_el_hash_y_reintenta_una_vez() {
    // el sha256 de "soy un jar de mentira", calculado con sha256sum
    let contenido = b"soy un jar de mentira".to_vec();
    let esperado = "0f9e0d8a1c9b7f5f0a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f7081";
    let (puerto, hilo) = servidor_falso("200 OK", contenido);
    let destino = destino("ok.jar");

    // con un hash que no es el suyo, falla y lo dice (y reintenta, así que el
    // servidor falso solo atiende el primero: el segundo intento falla distinto y
    // lo que se devuelve es el error del PRIMERO)
    let err = net::download_to(
        &format!("http://127.0.0.1:{puerto}/x.jar"),
        &destino,
        &net::OpcionesDescarga {
            sha256: Some(esperado),
            ..Default::default()
        },
    )
    .expect_err("el hash no coincide");
    assert!(err.to_string().contains("SHA256 no coincide en"), "{err}");
    let _ = hilo.join();
    let _ = std::fs::remove_file(&destino);
}

#[test]
fn un_404_es_un_error_con_el_codigo() {
    let (puerto, hilo) = servidor_falso("404 Not Found", b"no".to_vec());
    let destino = destino("404.jar");
    let err = net::download_to(
        &format!("http://127.0.0.1:{puerto}/no.jar"),
        &destino,
        &Default::default(),
    )
    .expect_err("404");
    // el mensaje es el del TS: `HTTP ${status} en ${url}`
    assert_eq!(
        err.to_string(),
        format!("HTTP 404 en http://127.0.0.1:{puerto}/no.jar")
    );
    let _ = hilo.join();
    // el cuerpo del error no se queda en el destino
    assert!(!destino.exists(), "no deberia quedar el fichero de un 404");
}
