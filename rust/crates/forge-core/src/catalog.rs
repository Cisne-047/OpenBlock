//! Catálogo: engines, plugins/mods de rendimiento y mapa Java/MC. Portado de
//! `src/catalog.ts`.

use crate::types::EngineId;

/// Tipo de loader que un engine expone a plugins/mods (`LoaderKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoaderKind {
    None,
    Bukkit,
    Paper,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
}

impl LoaderKind {
    /// Valor exacto del enum en JSON/TS.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bukkit => "bukkit",
            Self::Paper => "paper",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::Neoforge => "neoforge",
        }
    }

    /// Parsea un loader; `None` si no es conocido.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "bukkit" => Self::Bukkit,
            "paper" => Self::Paper,
            "fabric" => Self::Fabric,
            "quilt" => Self::Quilt,
            "forge" => Self::Forge,
            "neoforge" => Self::Neoforge,
            _ => return None,
        })
    }
}

impl std::fmt::Display for LoaderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Definición de un engine (`EngineDef`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDef {
    pub id: EngineId,
    pub label: &'static str,
    /// tipo de loader que expone a plugins/mods.
    pub loader: LoaderKind,
    pub desc: &'static str,
    /// spigot-compatible (bukkit/spigot/paper plugins).
    pub plugin_api: bool,
    /// soporta mods (fabric/forge/quilt/neoforge).
    pub mod_api: bool,
    /// hecho para multithreading real (regiones paralelas).
    pub multithread: bool,
    /// ¿el jar del engine se instala ejecutando un installer con java?
    pub installer: bool,
    /// carga modpacks nativamente (fabric/quilt/forge/neoforge).
    pub modpack_loader: bool,
}

/// Los ocho engines soportados (`ENGINES`).
///
/// En el TS `engineById` lanza si el id no existe; aquí el id es un enum cerrado,
/// así que ese caso no existe y la función es total: devuelve `&'static EngineDef`
/// sin `Result` ni `Option`. Es la primera de las divergencias anotadas en el
/// README del port, y la misma que DistroForge documenta para su `base_by_id`.
pub const ENGINES: &[EngineDef] = &[
    EngineDef {
        id: EngineId::Vanilla,
        label: "Vanilla",
        loader: LoaderKind::None,
        desc: "Server oficial de Mojang. Cero optimizaciones, cero plugins.",
        plugin_api: false,
        mod_api: false,
        multithread: false,
        installer: false,
        modpack_loader: false,
    },
    EngineDef {
        id: EngineId::Paper,
        label: "Paper",
        loader: LoaderKind::Paper,
        desc: "Fork de Spigot con miles de fixes y optimizaciones. El estándar.",
        plugin_api: true,
        mod_api: false,
        multithread: false,
        installer: false,
        modpack_loader: false,
    },
    EngineDef {
        id: EngineId::Purpur,
        label: "Purpur",
        loader: LoaderKind::Paper,
        desc: "Fork de Paper con más features de config. Mismo API de plugins.",
        plugin_api: true,
        mod_api: false,
        multithread: false,
        installer: false,
        modpack_loader: false,
    },
    EngineDef {
        id: EngineId::Folia,
        label: "Folia",
        loader: LoaderKind::Paper,
        desc: "Fork de Paper multithread por regiones. Máquinas con 16+ cores.",
        plugin_api: true,
        mod_api: false,
        multithread: true,
        installer: false,
        modpack_loader: false,
    },
    EngineDef {
        id: EngineId::Fabric,
        label: "Fabric",
        loader: LoaderKind::Fabric,
        desc: "Loader ligero de mods. Ideal con Lithium + C2ME.",
        plugin_api: false,
        mod_api: true,
        multithread: false,
        installer: false,
        modpack_loader: true,
    },
    EngineDef {
        id: EngineId::Quilt,
        label: "Quilt",
        loader: LoaderKind::Quilt,
        desc: "Fork de Fabric, compatible con mods de Fabric y Quilt.",
        plugin_api: false,
        mod_api: true,
        multithread: false,
        installer: false,
        modpack_loader: true,
    },
    EngineDef {
        id: EngineId::Forge,
        label: "Forge",
        loader: LoaderKind::Forge,
        desc: "El loader de mods clásico. Modpacks grandes (ATM, GTNH...).",
        plugin_api: false,
        mod_api: true,
        multithread: false,
        installer: true,
        modpack_loader: true,
    },
    EngineDef {
        id: EngineId::Neoforge,
        label: "NeoForge",
        loader: LoaderKind::Neoforge,
        desc: "Continuación moderna de Forge. MC 1.20.2+.",
        plugin_api: false,
        mod_api: true,
        multithread: false,
        installer: true,
        modpack_loader: true,
    },
];

/// Definición de un engine por id. Total, porque `EngineId` es cerrado.
pub fn engine_by_id(id: EngineId) -> &'static EngineDef {
    ENGINES
        .iter()
        .find(|e| e.id == id)
        .expect("todo EngineId tiene su EngineDef en ENGINES")
}

/// Item del catálogo de rendimiento (`PerfItem`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfItem {
    pub id: &'static str,
    pub label: &'static str,
    /// "plugin" o "mod".
    pub kind: &'static str,
    /// loaders compatibles.
    pub loaders: &'static [LoaderKind],
    /// versión MC mínima con la que funciona (inclusive).
    pub min_mc: &'static str,
    /// versión MC máxima con la que funciona (inclusive); "999" = sin tope.
    pub max_mc: &'static str,
    /// slug en Modrinth.
    pub slug: &'static str,
    pub reason: &'static str,
    pub default: bool,
}

/// Catálogo de rendimiento (`PERF_ITEMS`). Todo baja de Modrinth (API pública).
pub const PERF_ITEMS: &[PerfItem] = &[
    PerfItem {
        id: "spark",
        label: "spark",
        kind: "plugin",
        loaders: &[
            LoaderKind::Paper,
            LoaderKind::Fabric,
            LoaderKind::Forge,
            LoaderKind::Neoforge,
            LoaderKind::Quilt,
        ],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "spark",
        reason: "Profiler de CPU/mem/ticks. Diagnóstico de lag en vivo.",
        default: true,
    },
    PerfItem {
        id: "chunky",
        label: "Chunky",
        kind: "plugin",
        loaders: &[
            LoaderKind::Paper,
            LoaderKind::Fabric,
            LoaderKind::Forge,
            LoaderKind::Neoforge,
            LoaderKind::Quilt,
        ],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "chunky",
        reason: "Pregenera chunks antes del lanzamiento: cero lag de exploración.",
        default: true,
    },
    PerfItem {
        id: "chunkyborder",
        label: "ChunkyBorder",
        kind: "plugin",
        loaders: &[LoaderKind::Paper],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "chunkyborder",
        reason: "Borde de mundo: delimita la pregeneración y bloquea la fuga de chunks.",
        default: false,
    },
    PerfItem {
        id: "ferritecore",
        label: "FerriteCore",
        kind: "mod",
        loaders: &[
            LoaderKind::Fabric,
            LoaderKind::Quilt,
            LoaderKind::Forge,
            LoaderKind::Neoforge,
        ],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "ferrite-core",
        reason: "Reduce uso de RAM hasta ~40% en memoria de bloques/estados.",
        default: true,
    },
    PerfItem {
        id: "modernfix",
        label: "ModernFix",
        kind: "mod",
        loaders: &[
            LoaderKind::Fabric,
            LoaderKind::Quilt,
            LoaderKind::Forge,
            LoaderKind::Neoforge,
        ],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "modernfix",
        reason: "Acelera arranque y reduce RAM. Salvavidas en modpacks grandes.",
        default: true,
    },
    PerfItem {
        id: "lithium",
        label: "Lithium",
        kind: "mod",
        loaders: &[LoaderKind::Fabric, LoaderKind::Quilt, LoaderKind::Neoforge],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "lithium",
        reason: "Optimiza física/mobs/IA sin cambiar gameplay. +20-40% TPS.",
        default: true,
    },
    PerfItem {
        id: "c2me",
        label: "C2ME",
        kind: "mod",
        loaders: &[LoaderKind::Fabric, LoaderKind::Quilt],
        min_mc: "1.18",
        max_mc: "999",
        slug: "c2me-fabric",
        reason: "Generación de chunks en paralelo con todos los cores.",
        default: true,
    },
    PerfItem {
        id: "krypton",
        label: "Krypton",
        kind: "mod",
        loaders: &[LoaderKind::Fabric, LoaderKind::Quilt],
        min_mc: "1.16.5",
        max_mc: "999",
        slug: "krypton",
        reason: "Optimiza el stack de red (Netty). Menos CPU por paquete.",
        default: true,
    },
    PerfItem {
        id: "canary",
        label: "Canary",
        kind: "mod",
        loaders: &[LoaderKind::Forge],
        min_mc: "1.18.2",
        max_mc: "1.20.1",
        slug: "canary",
        reason: "Port de Lithium a Forge.",
        default: true,
    },
    PerfItem {
        id: "lazydfu",
        label: "LazyDFU",
        kind: "mod",
        loaders: &[LoaderKind::Fabric, LoaderKind::Quilt, LoaderKind::Forge],
        min_mc: "1.14",
        max_mc: "1.19.4",
        slug: "lazydfu",
        reason: "Diferir inicialización de DFU: arranque más rápido (1.19 y antes).",
        default: true,
    },
];

/// Items del catálogo que aplican a un loader y una versión de MC (`perfItemsFor`).
///
/// `maxMc === "999"` significa sin tope: en el TS es `Infinity`, aquí
/// simplemente no se compara el máximo.
pub fn perf_items_for(loader: LoaderKind, mc: &str) -> Vec<&'static PerfItem> {
    let v = version_key(parse_mc(mc));
    PERF_ITEMS
        .iter()
        .filter(|it| {
            if !it.loaders.contains(&loader) {
                return false;
            }
            let min = version_key(parse_mc(it.min_mc));
            if v < min {
                return false;
            }
            it.max_mc == "999" || v <= version_key(parse_mc(it.max_mc))
        })
        .collect()
}

/// `[mayor, menor, parche]` con 0 por defecto (`parseMc`).
///
/// Implementa el mismo regex `^(\d+)\.(\d+)(?:\.(\d+))?` que el TS, sin traer una
/// librería de expresiones regulares. Los cuatro detalles que importan, y los
/// cuatro comprobados contra el TS porque mis tres primeras suposiciones eran
/// falsas —la paridad las cazó una a una—:
///
/// - el primer grupo exige un punto después: un snapshot sin punto (`26w14a`) no
///   casa y devuelve ceros.
/// - mayor y menor toman **solo los dígitos iniciales** de su parte: el regex
///   retrocede, así que `1.2b.3` casa y da `[1, 2, 0]`.
/// - la parte del parche **no se busca**: el grupo opcional solo participa si hay
///   un punto justo tras los dígitos del menor. No se puede "saltar" el `b` de
///   `1.2b.3` para llegar al `.3`: el match acaba en el `2`.
/// - por eso `1.20.b` da `[1, 20, 0]` (hay punto, pero no dígitos detrás) y no cae
///   a ceros: el match ya había cuajado con mayor y menor.
pub fn parse_mc(v: &str) -> [u32; 3] {
    fn digitos_iniciales(s: &str) -> Option<u32> {
        let fin = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        s.get(..fin).filter(|d| !d.is_empty())?.parse().ok()
    }

    let Some((mayor_s, resto)) = v.split_once('.') else {
        return [0, 0, 0];
    };
    let Some(mayor) = digitos_iniciales(mayor_s) else {
        return [0, 0, 0];
    };
    let Some(menor) = digitos_iniciales(resto) else {
        return [0, 0, 0];
    };
    // dónde acaban los dígitos del menor: solo ahí se permite el punto del parche
    let fin_menor = resto
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(resto.len());
    let parche = resto
        .get(fin_menor..)
        .and_then(|s| s.strip_prefix('.'))
        .and_then(digitos_iniciales)
        .unwrap_or(0);

    [mayor, menor, parche]
}

/// Clave comparable de una versión (`versionKey`): `mayor*1e6 + menor*1e3 + parche`.
pub fn version_key(v: [u32; 3]) -> i64 {
    i64::from(v[0]) * 1_000_000 + i64::from(v[1]) * 1_000 + i64::from(v[2])
}

/// Versión de Java necesaria para una versión de Minecraft (`javaMajorFor`).
pub fn java_major_for(mc: &str) -> u32 {
    let v = parse_mc(mc);
    let key = version_key(v);
    // MC 26.x (versionado por fecha desde 2026) exige Java 25
    if key >= version_key([26, 0, 0]) {
        return 25;
    }
    // snapshots y futuros van con la más moderna conocida
    if v[0] > 1 {
        return 25;
    }
    if key >= version_key([1, 20, 5]) {
        return 21;
    }
    if key >= version_key([1, 17, 0]) {
        return 17;
    }
    8
}
