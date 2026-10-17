//! Núcleo generador de OpenBlock: `config → FileMap`, puro y sin E/S.
//!
//! Es el port de `src/` del proyecto TypeScript, con el mismo reparto que
//! DistroForge usa en su `forge-core`: aquí no entra una sola referencia a
//! `std::fs`, `std::io`, `std::process` ni `std::env`. Construir contenido es
//! puro y toda la E/S vive en el borde (el CLI).
//!
//! ## Estado del port
//!
//! Va por partes, y cada parte entra con su prueba de paridad byte a byte contra
//! el TS (`tests/parity.rs`), no "debería coincidir":
//!
//! - [x] `types` — el modelo de config, con el orden de campos idéntico al TS
//! - [x] `generators::jvm` — flags de Aikar y ZGC
//! - [x] `generators::properties` — `server.properties`
//! - [x] `generators::paper` — YAML de la familia Paper
//! - [x] `generators::ops` — firewall y backups
//! - [x] `generators::service` — systemd y WinSW
//! - [ ] `catalog` — engines, plugins/mods, mapa Java/MC
//! - [ ] `generators::scripts` — `start.sh`/`start.bat`/installers
//! - [ ] `generators` (raíz) — `generateProject` completo
//! - [ ] `hw` — detección de hardware y tiers
//! - [ ] `plan` y `api` — resolución de descargas contra las APIs

pub mod catalog;
pub mod generators;
pub mod hw;
pub mod modrinth;
pub mod plan;
pub mod rcon;
pub mod types;

pub use catalog::LoaderKind;
pub use types::{EngineId, PerfProfile, ServerConfig, TargetOs};

/// Versión del núcleo.
///
/// Sale del `Cargo.toml`, que es el mismo 0.1.0 que el `package.json` del TS: en
/// el TS la versión está declarada una vez en `src/version.ts` y un test la
/// compara con `package.json`; aquí la declara el propio manifiesto, así que no
/// puede desincronizarse.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identificador con el que nos presentamos a las APIs de terceros.
pub const USER_AGENT: &str = concat!(
    "OpenBlock/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/cisne-047)"
);

/// Mapa de ficheros generados: ruta relativa → contenido.
///
/// En el TS es un `Record<string, string>`; aquí un `BTreeMap` para que el orden
/// de recorrido sea determinista. DistroForge usa `BTreeMap` por el mismo motivo:
/// el zip y las pruebas de paridad salen iguales en cada ejecución.
pub type FileMap = std::collections::BTreeMap<String, String>;
