//! Mojang piston-meta, portado de `src/api/mojang.ts`.
//!
//! El manifiesto de versiones se pide **una vez** y se cachea: es un JSON de
//! varios cientos de KB que no cambia entre llamadas dentro de la misma ejecución.
//! El TS usa una variable de módulo; aquí un `OnceLock`, que es lo mismo pero sin
//! mutabilidad global.

use crate::net;
use serde::Deserialize;
use std::sync::OnceLock;

/// Endpoint del manifiesto (igual que el `MANIFEST` del TS).
pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Una versión del manifiesto (`McVersion`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersion {
    pub id: String,
    /// "release", "snapshot", "old_beta" u "old_alpha".
    #[serde(rename = "type")]
    pub tipo: String,
    /// URL del JSON de la versión, con sus descargas.
    pub url: String,
    pub release_time: String,
}

/// El manifiesto completo (`Manifest`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<McVersion>,
}

/// Última release y último snapshot (`Manifest.latest`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

/// Descarga del server vanilla (`ServerDownload`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerDownload {
    pub url: String,
    pub sha1: String,
}

/// Caché del manifiesto: una sola descarga por proceso.
static MANIFIESTO: OnceLock<Manifest> = OnceLock::new();

/// El manifiesto de versiones, cacheado (`versionManifest`).
///
/// Devuelve `Result` y no lanza: el TS deja que el `fetch` reviente con una
/// excepción, pero aquí quien llama es el CLI y un error de red no puede tumbar el
/// proceso antes de imprimir nada útil.
pub fn version_manifest() -> Result<&'static Manifest, net::Error> {
    if let Some(m) = MANIFIESTO.get() {
        return Ok(m);
    }
    let m: Manifest = net::get_json(MANIFEST_URL)?;
    // si dos hilos llegan a la vez, uno gana y el otro recibe el mismo valor
    Ok(MANIFIESTO.get_or_init(|| m))
}

/// Última release estable (`latestRelease`).
pub fn latest_release() -> Result<&'static str, net::Error> {
    Ok(&version_manifest()?.latest.release)
}

/// Las primeras `limit` releases estables (`listReleases`).
///
/// El TS filtra por `type === "release"` y corta con `slice(0, limit)`: los
/// snapshots quedan fuera y el orden es el del manifiesto, que va de nuevo a viejo.
pub fn list_releases(limit: usize) -> Result<Vec<String>, net::Error> {
    Ok(filtrar_releases(version_manifest()?, limit))
}

/// El filtro de `listReleases`, separado para poder probarlo sin red.
pub fn filtrar_releases(m: &Manifest, limit: usize) -> Vec<String> {
    m.versions
        .iter()
        .filter(|v| v.tipo == "release")
        .take(limit)
        .map(|v| v.id.clone())
        .collect()
}

/// Encuentra una versión por id (`m.versions.find`).
pub fn version_por_id<'a>(m: &'a Manifest, mc: &str) -> Option<&'a McVersion> {
    m.versions.iter().find(|v| v.id == mc)
}

/// El `server.jar` de una versión (`vanillaServerFor`).
///
/// Necesita una segunda petición —el JSON de la versión— así que no se puede
/// cachear junto al manifiesto sin traerse todas las versiones.
pub fn vanilla_server_for(mc: &str) -> Result<ServerDownload, Error> {
    let m = version_manifest()?;
    let v = version_por_id(m, mc).ok_or_else(|| Error::VersionDesconocida(mc.to_string()))?;
    let meta: MetaVersion = net::get_json(&v.url)?;
    let server = meta
        .downloads
        .and_then(|d| d.server)
        .ok_or_else(|| Error::SinServer(mc.to_string()))?;
    Ok(server)
}

/// Error de `vanillaServerFor`: mensajes del TS, no códigos.
#[derive(Debug)]
pub enum Error {
    Red(net::Error),
    /// `Versión de Minecraft desconocida: {mc}`
    VersionDesconocida(String),
    /// `MC {mc} no tiene server.jar publicado`
    SinServer(String),
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
            Self::VersionDesconocida(mc) => write!(f, "Versión de Minecraft desconocida: {mc}"),
            Self::SinServer(mc) => write!(f, "MC {mc} no tiene server.jar publicado"),
        }
    }
}

impl std::error::Error for Error {}

/// Solo la parte del JSON de versión que se usa.
#[derive(Debug, Clone, Deserialize)]
struct MetaVersion {
    downloads: Option<Descargas>,
}

/// `downloads` del JSON de versión.
#[derive(Debug, Clone, Deserialize)]
struct Descargas {
    server: Option<ServerDownload>,
}
