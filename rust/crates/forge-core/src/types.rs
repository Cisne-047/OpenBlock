//! Tipos de configuración, portados de `src/types.ts`.
//!
//! Todos los structs derivan `Serialize`/`Deserialize` con `rename_all =
//! "camelCase"` para que el JSON sea idéntico al del TS (`openblock.json`). El
//! orden de declaración de los campos **replica el de las interfaces TS**, porque
//! `JSON.stringify` y `serde_json` serializan en ese orden: es lo que permite
//! comparar las dos implementaciones con un `diff`.
//!
//! Al deserializar, los campos que faltan toman valores coherentes con los
//! fallbacks del TS (el `?? 25575` de `rcon.port`, el `?? 24` de
//! `backup.intervalHours`...), y los desconocidos se ignoran sin protestar: el
//! TS tampoco tiene `strict` en el round-trip.

use serde::{Deserialize, Serialize};

/// Motor del server (`EngineId`).
///
/// El TS usa una unión de strings; aquí es un enum cerrado, así que un id
/// inventado no compila ni deserializa en vez de propagarse hasta el generador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EngineId {
    Vanilla,
    #[default]
    Paper,
    Purpur,
    Folia,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
}

impl EngineId {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Paper => "paper",
            Self::Purpur => "purpur",
            Self::Folia => "folia",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::Neoforge => "neoforge",
        }
    }

    /// Parsea un id de engine; `None` si no es ninguno conocido.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "vanilla" => Self::Vanilla,
            "paper" => Self::Paper,
            "purpur" => Self::Purpur,
            "folia" => Self::Folia,
            "fabric" => Self::Fabric,
            "quilt" => Self::Quilt,
            "forge" => Self::Forge,
            "neoforge" => Self::Neoforge,
            _ => return None,
        })
    }

    /// Engines que cargan mods (reciben `-XX:MaxMetaspaceSize` y `mods/`).
    pub fn is_modded(self) -> bool {
        matches!(
            self,
            Self::Fabric | Self::Quilt | Self::Forge | Self::Neoforge
        )
    }
}

impl std::fmt::Display for EngineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Perfil de rendimiento (`PerfProfile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PerfProfile {
    Conservative,
    #[default]
    Balanced,
    Aggressive,
}

impl PerfProfile {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
        }
    }

    /// Parsea un perfil; `None` si no es conocido.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "conservative" => Self::Conservative,
            "balanced" => Self::Balanced,
            "aggressive" => Self::Aggressive,
            _ => return None,
        })
    }
}

impl std::fmt::Display for PerfProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tipo de disco detectado (`DiskKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiskKind {
    Nvme,
    Ssd,
    Hdd,
    #[default]
    Unknown,
}

impl DiskKind {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nvme => "nvme",
            Self::Ssd => "ssd",
            Self::Hdd => "hdd",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for DiskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Dificultad del mundo (`ServerConfig.difficulty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Modo de juego (`ServerConfig.gamemode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Gamemode {
    #[default]
    Survival,
    Creative,
    Adventure,
}

impl Gamemode {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
        }
    }
}

impl std::fmt::Display for Gamemode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tipo de mundo (`ServerConfig.worldType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorldType {
    #[default]
    Normal,
    Flat,
    #[serde(rename = "large_biomes")]
    LargeBiomes,
    Amplified,
}

impl WorldType {
    /// Valor exacto del enum en JSON/TS (con guion bajo, como `server.properties`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Flat => "flat",
            Self::LargeBiomes => "large_biomes",
            Self::Amplified => "amplified",
        }
    }
}

impl std::fmt::Display for WorldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// SO objetivo de los scripts generados (`ServerConfig.targetOS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TargetOs {
    Linux,
    Windows,
    #[default]
    Both,
}

impl TargetOs {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Both => "both",
        }
    }
}

impl std::fmt::Display for TargetOs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tipo de modpack (`ModpackRef.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModpackKind {
    #[default]
    None,
    Modrinth,
    Curseforge,
}

impl ModpackKind {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
        }
    }
}

impl std::fmt::Display for ModpackKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// id de tier (`TierId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TierId {
    Potato,
    Low,
    #[default]
    Mid,
    High,
    Enthusiast,
}

impl TierId {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Potato => "potato",
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
            Self::Enthusiast => "enthusiast",
        }
    }
}

impl std::fmt::Display for TierId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Resultado del benchmark (`BenchmarkResult`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    /// ops de CPU por core por ms, normalizado.
    pub cpu_score: f64,
    /// MB/s de escritura secuencial.
    #[serde(rename = "diskMBs")]
    pub disk_m_bs: f64,
    pub duration_ms: f64,
}

/// Perfil de hardware (`HardwareProfile`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfile {
    /// "windows" o "linux", como el `process.platform` del TS.
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub hostname: String,
    pub cpu_model: String,
    /// cores lógicos.
    pub cores: u32,
    #[serde(rename = "ramTotalMB")]
    pub ram_total_mb: u64,
    #[serde(rename = "ramFreeMB")]
    pub ram_free_mb: u64,
    #[serde(rename = "swapTotalMB")]
    pub swap_total_mb: u64,
    pub disk_kind: DiskKind,
    #[serde(rename = "diskFreeMB")]
    pub disk_free_mb: u64,
    pub virtualized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bench: Option<BenchmarkResult>,
}

/// Tier de hardware (`Tier`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tier {
    pub id: TierId,
    pub label: String,
    pub max_players: u32,
    pub view_distance: u32,
    pub simulation_distance: u32,
    pub pregen_radius: u32,
    pub recommended_engine: EngineId,
    /// "g1" o "zgc".
    pub gc: String,
    /// % de RAM total que conviene asignar si el server está dedicado.
    pub dedicated_ram_pct: f64,
}

/// Referencia a un modpack (`ModpackRef`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModpackRef {
    pub kind: ModpackKind,
    /// URL de la página del modpack o del `.mrpack`.
    pub url: String,
}

/// Plugin/mod propio del usuario (`CustomItem`).
///
/// `url` y `path` **no** se omiten cuando faltan: el TS los emite siempre (en el
/// objeto va una clave con `undefined`, que `JSON.stringify` convierte en
/// `null`), así que aquí van como `Option` sin `skip_serializing_if` para que el
/// JSON coincida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomItem {
    pub name: String,
    /// "plugin" o "mod".
    pub kind: String,
    /// URL http(s) de descarga directa (jar).
    pub url: Option<String>,
    /// ruta local al jar (se copia al server).
    pub path: Option<String>,
}

/// Config de backups (`ServerConfig.backup`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval_hours: u32,
    pub keep: u32,
}

impl Default for BackupConfig {
    fn default() -> Self {
        // El TS reparte `?? 24` y `?? 7` por el código: estos son los coherentes.
        Self {
            enabled: true,
            interval_hours: 24,
            keep: 7,
        }
    }
}

/// Config del servicio (`ServerConfig.service`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Config de RCON (`ServerConfig.rcon`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RconConfig {
    pub enabled: bool,
    pub password: String,
    pub port: u16,
}

impl Default for RconConfig {
    fn default() -> Self {
        // El TS usa `?? 25575` en el modo avanzado.
        Self {
            enabled: true,
            password: String::new(),
            port: 25575,
        }
    }
}

/// Config completa del server (`ServerConfig`).
///
/// El orden de los campos replica el de la interfaz TS y el del objeto que
/// construye `buildAdvancedConfig`, de modo que `openblock.json` (JSON con 2
/// espacios de indentación y salto final) sale byte a byte idéntico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub name: String,
    pub mc_version: String,
    pub engine: EngineId,
    pub server_dir: String,
    /// `-Xmx` en MB. El JSON del TS dice `ramMB`, no `ramMb`: serde no adivina MB.
    #[serde(rename = "ramMB")]
    pub ram_mb: u32,
    pub players: u32,
    pub view_distance: u32,
    pub simulation_distance: u32,
    pub port: u16,
    pub motd: String,
    pub difficulty: Difficulty,
    pub gamemode: Gamemode,
    pub world_type: WorldType,
    pub seed: String,
    pub online_mode: bool,
    pub whitelist: bool,
    pub pvp: bool,
    pub hardcore: bool,
    pub modpack: ModpackRef,
    pub performance: PerfProfile,
    /// pregeneración con Chunky; 0 = off.
    pub pregen_radius: u32,
    pub backup: BackupConfig,
    pub service: ServiceConfig,
    pub rcon: RconConfig,
    /// ruta java explícita; ausente = autogestionado por OpenBlock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
    /// chunkyborder junto a chunky.
    pub chunkyborder: bool,
    /// SO objetivo de los scripts generados. El JSON dice `targetOS`.
    #[serde(rename = "targetOS")]
    pub target_os: TargetOs,
    /// build exacta del engine: paper/purpur/folia = nº de build; forge/neoforge = build del loader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// build exacta del loader (fabric/quilt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader_build: Option<String>,
    /// versión del installer (fabric/quilt); ausente = latest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer_build: Option<String>,
    /// plugins/mods propios del usuario.
    pub custom_items: Vec<CustomItem>,
}

impl ServerConfig {
    /// `openblock.json` con la misma forma que el TS: 2 espacios y salto final.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        s.push('\n');
        s
    }
}
