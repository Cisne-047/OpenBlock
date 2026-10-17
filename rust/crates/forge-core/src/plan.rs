//! Manifiesto de descargas, portado de `src/plan.ts`.
//!
//! En el TS lo resuelven las APIs (`api/`) ANTES de generar el proyecto, para que
//! `apply` y los `install.sh`/`install.ps1` del zip usen las mismas URLs y hashes.
//! Aquí va solo el **modelo**, que es lo que necesitan los generadores: resolverlo
//! contra PaperMC/Modrinth es el siguiente tramo del port y necesita red, así que
//! no puede vivir en un núcleo puro.

use serde::{Deserialize, Serialize};

/// Qué es cada descarga (`DownloadKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadKind {
    Engine,
    Plugin,
    Mod,
    ModpackFile,
}

impl DownloadKind {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Plugin => "plugin",
            Self::Mod => "mod",
            Self::ModpackFile => "modpack-file",
        }
    }
}

/// Una descarga del plan (`DownloadEntry`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEntry {
    pub kind: DownloadKind,
    pub label: String,
    pub url: String,
    /// ruta relativa dentro del server; vacío para CurseForge: se deduce del nombre.
    pub dest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

impl DownloadEntry {
    /// El hash que acaba en los scripts: sha256 y, si no, sha512 (`?? ""` en el TS).
    pub fn script_sum(&self) -> &str {
        self.sha256
            .as_deref()
            .or(self.sha512.as_deref())
            .unwrap_or("")
    }
}

/// Installer que hay que ejecutar con java antes del primer arranque.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installer {
    pub jar: String,
    /// "forge" o "neoforge". En el TS es una unión de strings.
    pub engine: String,
}

/// Copia de un fichero local del usuario al server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCopy {
    pub src: String,
    pub dest: String,
}

/// Manifiesto completo (`DownloadPlan`).
///
/// **El orden de los campos es el del JSON que escribe el TS**, y no es el del
/// struct original de la interfaz: `buildDownloadPlan` construye un objeto parcial
/// (`engineFilename`, `engineResolved`, y según el engine `loaderResolved` o
/// `installer`), le añade `unresolvedCf` si hace falta y hace
/// `{ ...plan, entries, localCopies }`. `JSON.stringify` escribe en orden de
/// inserción, así que el `plan.json` del producto empieza por `engineFilename`.
/// Aquí se declara igual para que los dos ficheros se puedan comparar con un diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPlan {
    pub engine_filename: String,
    pub engine_resolved: String,
    /// loader version detectada (fabric/quilt) para trazabilidad.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader_resolved: Option<String>,
    /// jar installer que hay que ejecutar con java antes del primer arranque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<Installer>,
    /// modpacks CurseForge sin API key: pendientes de resolver a mano.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_cf: Option<i32>,
    pub entries: Vec<DownloadEntry>,
    /// ficheros locales del usuario a copiar al server.
    pub local_copies: Vec<LocalCopy>,
}
