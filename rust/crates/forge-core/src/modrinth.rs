//! Parsers de Modrinth y CurseForge, portados de `src/api/modrinth.ts`.
//!
//! Solo la parte **pura**: interpretar el `.mrpack` y el manifest del zip de
//! CurseForge. Las funciones que piden a la API viven en `forge-api`, porque
//! necesitan red. El reparto es el mismo que en el TS, donde estas cuatro no
//! llaman a `fetch`.

use serde::Deserialize;

// ---------- Modrinth: ficheros y versiones ----------

/// Un fichero de una versión (`ModrinthFile`).
///
/// Los opcionales se **omiten** al serializar, como el `undefined` de
/// `JSON.stringify`: `primaryFile` devuelve el objeto de la API tal cual, así que
/// una clave que no venía no puede reaparecer como `null`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hashes: Option<Hashes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

/// Hashes de un fichero. `sha1` y `sha512` pueden faltar por separado.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, serde::Serialize)]
pub struct Hashes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

/// Una versión publicada (`ModrinthVersion`).
///
/// Los nombres van en `snake_case` porque así los manda la API: aquí no hay
/// `rename_all` que valga.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<ModrinthFile>,
}

/// El fichero principal de una versión (`primaryFile`).
///
/// El `?? v.files[0]` del TS: si ninguno está marcado como primario se coge el
/// primero, y si no hay ficheros... el TS devolvería `undefined` y reventaría al
/// usarlo. Aquí es `None` y quien llama decide.
pub fn primary_file(v: &ModrinthVersion) -> Option<&ModrinthFile> {
    v.files
        .iter()
        .find(|f| f.primary == Some(true))
        .or_else(|| v.files.first())
}

// ---------- Modrinth: modpacks .mrpack ----------

/// Un fichero dentro de un `.mrpack` (`MrpackEntry`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MrpackEntry {
    pub path: String,
    /// A qué lado va: `downloads` es del manifest, `env` dice si es de cliente o
    /// de servidor.
    #[serde(default)]
    pub env: Option<MrpackEnv>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub hashes: Hashes,
    #[serde(default, rename = "fileSize")]
    pub file_size: u64,
}

/// `env` de una entrada: qué lado necesita el fichero.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MrpackEnv {
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
}

/// El índice de un `.mrpack` (`MrpackIndex`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MrpackIndex {
    #[serde(rename = "formatVersion")]
    pub format_version: i64,
    #[serde(default)]
    pub game: String,
    #[serde(rename = "versionId")]
    pub version_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub files: Vec<MrpackEntry>,
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, String>,
}

/// Error de los parsers, con los mensajes del TS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `Formato .mrpack no soportado: {0}`
    FormatoMrpack(i64),
    /// `No es un manifest de modpack CurseForge: {0}`
    NoEsModpackCf(String),
    /// El JSON no se pudo interpretar.
    Json(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FormatoMrpack(v) => write!(f, "Formato .mrpack no soportado: {v}"),
            Self::NoEsModpackCf(t) => write!(f, "No es un manifest de modpack CurseForge: {t}"),
            Self::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Parsea el índice de un `.mrpack` (`parseMrpackIndex`).
///
/// Solo acepta `formatVersion === 1`: el TS lo compara con `!==` y lanza, así que
/// una versión futura del formato falla en vez de interpretarse mal.
pub fn parse_mrpack_index(index_json: &str) -> Result<MrpackIndex, Error> {
    let idx: MrpackIndex =
        serde_json::from_str(index_json).map_err(|e| Error::Json(e.to_string()))?;
    if idx.format_version != 1 {
        return Err(Error::FormatoMrpack(idx.format_version));
    }
    Ok(idx)
}

/// Los ficheros que van al servidor (`mrpackServerFiles`).
///
/// El filtro del TS es `f.env?.server !== "unsupported"`, y el `?.` importa: una
/// entrada **sin** `env` no es "unsupported", así que **se incluye**. Es lo que
/// mete `config/optimizado.toml` en el servidor sin que el pack diga nada, y una
/// versión "arreglada" que excluyera las entradas sin `env` rompería los packs.
pub fn mrpack_server_files(idx: &MrpackIndex) -> Vec<&MrpackEntry> {
    idx.files
        .iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.server.as_deref()) != Some("unsupported"))
        .collect()
}

// ---------- CurseForge ----------

/// El manifest del zip exportado por el launcher (`CfManifest`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CfManifest {
    pub minecraft: CfMinecraft,
    #[serde(rename = "manifestType")]
    pub manifest_type: String,
    #[serde(rename = "manifestVersion", default)]
    pub manifest_version: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub files: Vec<CfFile>,
    #[serde(default)]
    pub overrides: String,
}

/// Bloque `minecraft` del manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CfMinecraft {
    #[serde(default)]
    pub version: String,
    #[serde(rename = "modLoaders", default)]
    pub mod_loaders: Vec<CfLoader>,
}

/// Un loader del manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CfLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

/// Un fichero del manifest (solo para contar: los ids se resuelven con API key).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CfFile {
    #[serde(rename = "projectID")]
    pub project_id: i64,
    #[serde(rename = "fileID")]
    pub file_id: i64,
    #[serde(default)]
    pub required: bool,
}

/// Parsea el manifest de CurseForge (`parseCfManifest`).
pub fn parse_cf_manifest(json: &str) -> Result<CfManifest, Error> {
    let m: CfManifest = serde_json::from_str(json).map_err(|e| Error::Json(e.to_string()))?;
    if m.manifest_type != "minecraftModpack" {
        return Err(Error::NoEsModpackCf(m.manifest_type));
    }
    Ok(m)
}

/// Engine y build a partir del loader primario (`cfLoaderKind`).
///
/// Devuelve `None` si no hay loader primario o si no es Forge ni NeoForge (Fabric
/// en un manifest de CF, por ejemplo): el TS también devuelve `null` ahí.
pub fn cf_loader_kind(manifest: &CfManifest) -> Option<(&'static str, String)> {
    let id = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .map(|l| l.id.as_str())
        .unwrap_or("");
    if let Some(resto) = id.strip_prefix("forge-") {
        return Some(("forge", resto.to_string()));
    }
    if let Some(resto) = id.strip_prefix("neoforge-") {
        return Some(("neoforge", resto.to_string()));
    }
    None
}
