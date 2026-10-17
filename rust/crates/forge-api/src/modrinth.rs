//! Cliente de Modrinth: la mitad que va a la red, portado de `src/api/modrinth.ts`.
//!
//! Los parsers de modpack viven en `forge_core::modrinth` porque son puros; aquí
//! está lo que pide y elige: la query de `latestVersion`, la primera versión de
//! la lista y el fichero primario. Esas tres partes son puras de verdad y tienen
//! golden, que es lo que el `plan` necesita para no depender de una API ajena.

use crate::net;
use crate::url;
use serde::Deserialize;
use serde_json::Value;

/// Base de la API v2 (`MR` en el TS).
pub const MR: &str = "https://api.modrinth.com/v2";

/// Error del cliente: o red, o una respuesta que no cumple lo mínimo.
#[derive(Debug)]
pub enum Error {
    Red(net::Error),
    Mensaje(String),
}

impl From<net::Error> for Error {
    fn from(e: net::Error) -> Self {
        Self::Red(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Red(e) => write!(f, "{e}"),
            Self::Mensaje(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

/// Un fichero de una versión (`ModrinthFile`).
///
/// El orden y los `skip_serializing_if` replican el del TS: `JSON.stringify`
/// omite las claves `undefined` y escribe en orden de declaración, así que los
/// campos opcionales que no vienen en la respuesta no pueden salir como `null`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashes: Option<Hashes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

/// Los hashes que publica Modrinth (`hashes`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Hashes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

/// Una versión publicada (`ModrinthVersion`). Los nombres van en `snake_case`
/// porque así los manda la API: aquí no hay `rename_all` que valga.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

/// Pide la última versión de un proyecto que cumpla los filtros.
pub fn latest_version(
    slug: &str,
    game_versions: Option<&[String]>,
    loaders: Option<&[String]>,
) -> Result<ModrinthVersion, Error> {
    let destino = format!(
        "{MR}/project/{}/version?{}",
        url::encode_uri_component(slug),
        latest_version_params(game_versions, loaders)
    );
    let list: Value = net::get_json(&destino)?;
    modrinth_version_from(&list, slug)
}

/// La query de `latestVersion`.
///
/// `URLSearchParams` **no** escapa como `encodeURIComponent`: deja pasar `*`,
/// `~`, `-`, `.`, `_` y escribe el espacio como `+`. Y el valor va con
/// `JSON.stringify`, así que los corchetes y las comillas se escapan tal cual.
pub fn latest_version_params(
    game_versions: Option<&[String]>,
    loaders: Option<&[String]>,
) -> String {
    let mut pares: Vec<String> = Vec::new();
    if let Some(g) = game_versions.filter(|g| !g.is_empty()) {
        let json = serde_json::to_string(g).unwrap_or_default();
        pares.push(format!("game_versions={}", form_encode(&json)));
    }
    if let Some(l) = loaders.filter(|l| !l.is_empty()) {
        let json = serde_json::to_string(l).unwrap_or_default();
        pares.push(format!("loaders={}", form_encode(&json)));
    }
    pares.join("&")
}

/// El codificador de `application/x-www-form-urlencoded`: el conjunto seguro es
/// `A-Za-z0-9*-._` y el espacio va como `+`.
///
/// Cuidado con `~`: **sí** se escapa aquí (`%7E`) aunque `encodeURIComponent` lo
/// deje pasar. Lo cazó el golden, no la memoria: mi primera versión lo dejaba
/// fuera y el caso «valores raros» cantó la diferencia.
fn form_encode(s: &str) -> String {
    let mut salida = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b' ' => salida.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                salida.push(b as char)
            }
            _ => salida.push_str(&format!("%{b:02X}")),
        }
    }
    salida
}

/// La primera versión de la lista; sin lista o vacía, el error del TS.
pub fn modrinth_version_from(list: &Value, slug: &str) -> Result<ModrinthVersion, Error> {
    let vacia = || Error::Mensaje(format!("Modrinth: sin versión de {slug} para esos filtros"));
    let arr = list.as_array().ok_or_else(vacia)?;
    let primera = arr.first().ok_or_else(vacia)?;
    serde_json::from_value(primera.clone())
        .map_err(|e| Error::Mensaje(format!("respuesta de modrinth inesperada: {e}")))
}

/// El fichero marcado `primary`, o el primero. `None` solo si no hay ficheros.
pub fn primary_file(v: &ModrinthVersion) -> Option<&ModrinthFile> {
    v.files
        .iter()
        .find(|f| f.primary == Some(true))
        .or_else(|| v.files.first())
}

/// El modpack de una URL de Modrinth: su versión más reciente y el fichero.
pub fn modrinth_modpack_file(url: &str) -> Result<(ModrinthFile, ModrinthVersion), Error> {
    let slug = modrinth_modpack_slug(url);
    let version = latest_version(&slug, None, None)?;
    let file = primary_file(&version)
        .cloned()
        .ok_or_else(|| Error::Mensaje(format!("Modrinth: {slug} no tiene ficheros")))?;
    Ok((file, version))
}

/// El slug de una URL de Modrinth, o la propia URL si no lo parece.
///
/// El `\w` del TS es ASCII (`[A-Za-z0-9_]`); el de `regex` en Rust sería
/// Unicode, así que la clase va escrita a mano.
pub fn modrinth_modpack_slug(url: &str) -> String {
    let re = regex::Regex::new(r"modrinth\.com/(?:modpack|project)/([A-Za-z0-9_-]+)")
        .expect("el patrón es constante y válido");
    match re.captures(url) {
        Some(c) => c[1].to_string(),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{form_encode, latest_version_params};

    #[test]
    fn el_espacio_va_como_mas_y_lo_demas_escapa() {
        assert_eq!(form_encode("a b"), "a+b");
        // `*` pasa y `~` no: la diferencia con `encodeURIComponent`
        assert_eq!(form_encode("a*b~"), "a*b%7E");
        assert_eq!(form_encode("!()'"), "%21%28%29%27");
        assert_eq!(form_encode(r#"["1.20.1"]"#), "%5B%221.20.1%22%5D");
    }

    #[test]
    fn la_query_mantiene_el_orden_y_se_salta_las_listas_vacias() {
        let g = vec!["1.20.1".to_string()];
        let l = vec!["paper".to_string()];
        assert_eq!(
            latest_version_params(Some(&g), Some(&l)),
            "game_versions=%5B%221.20.1%22%5D&loaders=%5B%22paper%22%5D"
        );
        assert_eq!(
            latest_version_params(Some(&g), None),
            "game_versions=%5B%221.20.1%22%5D"
        );
        assert_eq!(latest_version_params(None, Some(&[])), "");
    }
}
