import type { EngineId, LoaderKind } from "./types";

export interface EngineDef {
  id: EngineId;
  label: string;
  /** tipo de loader que expone a plugins/mods */
  loader: LoaderKind;
  desc: string;
  /** swigot-compatible (bukkit/spigot/paper plugins) */
  pluginApi: boolean;
  /** soporta mods (fabric/forge/quilt/neoforge) */
  modApi: boolean;
  /** hecho para multithreading real (regiones paralelas) */
  multithread: boolean;
  /** ¿el jar del engine se instala ejecutando un installer con java? */
  installer: boolean;
  /** carga modpacks nativamente (fabric/quilt/forge/neoforge) */
  modpackLoader: boolean;
}

export const ENGINES: EngineDef[] = [
  {
    id: "vanilla",
    label: "Vanilla",
    loader: "none",
    desc: "Server oficial de Mojang. Cero optimizaciones, cero plugins.",
    pluginApi: false,
    modApi: false,
    multithread: false,
    installer: false,
    modpackLoader: false,
  },
  {
    id: "paper",
    label: "Paper",
    loader: "paper",
    desc: "Fork de Spigot con miles de fixes y optimizaciones. El estándar.",
    pluginApi: true,
    modApi: false,
    multithread: false,
    installer: false,
    modpackLoader: false,
  },
  {
    id: "purpur",
    label: "Purpur",
    loader: "paper",
    desc: "Fork de Paper con más features de config. Mismo API de plugins.",
    pluginApi: true,
    modApi: false,
    multithread: false,
    installer: false,
    modpackLoader: false,
  },
  {
    id: "folia",
    label: "Folia",
    loader: "paper",
    desc: "Fork de Paper multithread por regiones. Máquinas con 16+ cores.",
    pluginApi: true,
    modApi: false,
    multithread: true,
    installer: false,
    modpackLoader: false,
  },
  {
    id: "fabric",
    label: "Fabric",
    loader: "fabric",
    desc: "Loader ligero de mods. Ideal con Lithium + C2ME.",
    pluginApi: false,
    modApi: true,
    multithread: false,
    installer: false,
    modpackLoader: true,
  },
  {
    id: "quilt",
    label: "Quilt",
    loader: "quilt",
    desc: "Fork de Fabric, compatible con mods de Fabric y Quilt.",
    pluginApi: false,
    modApi: true,
    multithread: false,
    installer: false,
    modpackLoader: true,
  },
  {
    id: "forge",
    label: "Forge",
    loader: "forge",
    desc: "El loader de mods clásico. Modpacks grandes (ATM, GTNH...).",
    pluginApi: false,
    modApi: true,
    multithread: false,
    installer: true,
    modpackLoader: true,
  },
  {
    id: "neoforge",
    label: "NeoForge",
    loader: "neoforge",
    desc: "Continuación moderna de Forge. MC 1.20.2+.",
    pluginApi: false,
    modApi: true,
    multithread: false,
    installer: true,
    modpackLoader: true,
  },
];

export const engineById = (id: EngineId): EngineDef => {
  const e = ENGINES.find((e) => e.id === id);
  if (!e) throw new Error(`Engine desconocido: ${id}`);
  return e;
};

export interface PerfItem {
  id: string;
  label: string;
  kind: "plugin" | "mod";
  /** loaders compatibles */
  loaders: LoaderKind[];
  /** versiones MC mínima y máxima con las que funciona (inclusive) */
  minMc: string;
  maxMc: string;
  /** slug en Modrinth */
  slug: string;
  reason: string;
  default: boolean;
}

/**
 * Catálogo de rendimiento. Todo baja de Modrinth (API pública, sin key).
 * maxMc "999" = sin tope conocido.
 */
export const PERF_ITEMS: PerfItem[] = [
  {
    id: "spark",
    label: "spark",
    kind: "plugin",
    loaders: ["paper", "fabric", "forge", "neoforge", "quilt"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "spark",
    reason: "Profiler de CPU/mem/ticks. Diagnóstico de lag en vivo.",
    default: true,
  },
  {
    id: "chunky",
    label: "Chunky",
    kind: "plugin",
    loaders: ["paper", "fabric", "forge", "neoforge", "quilt"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "chunky",
    reason: "Pregenera chunks antes del lanzamiento: cero lag de exploración.",
    default: true,
  },
  {
    id: "chunkyborder",
    label: "ChunkyBorder",
    kind: "plugin",
    loaders: ["paper"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "chunkyborder",
    reason: "Borde de mundo: delimita la pregeneración y bloquea la fuga de chunks.",
    default: false,
  },
  {
    id: "ferritecore",
    label: "FerriteCore",
    kind: "mod",
    loaders: ["fabric", "quilt", "forge", "neoforge"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "ferrite-core",
    reason: "Reduce uso de RAM hasta ~40% en memoria de bloques/estados.",
    default: true,
  },
  {
    id: "modernfix",
    label: "ModernFix",
    kind: "mod",
    loaders: ["fabric", "quilt", "forge", "neoforge"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "modernfix",
    reason: "Acelera arranque y reduce RAM. Salvavidas en modpacks grandes.",
    default: true,
  },
  {
    id: "lithium",
    label: "Lithium",
    kind: "mod",
    loaders: ["fabric", "quilt", "neoforge"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "lithium",
    reason: "Optimiza física/mobs/IA sin cambiar gameplay. +20-40% TPS.",
    default: true,
  },
  {
    id: "c2me",
    label: "C2ME",
    kind: "mod",
    loaders: ["fabric", "quilt"],
    minMc: "1.18",
    maxMc: "999",
    slug: "c2me-fabric",
    reason: "Generación de chunks en paralelo con todos los cores.",
    default: true,
  },
  {
    id: "krypton",
    label: "Krypton",
    kind: "mod",
    loaders: ["fabric", "quilt"],
    minMc: "1.16.5",
    maxMc: "999",
    slug: "krypton",
    reason: "Optimiza el stack de red (Netty). Menos CPU por paquete.",
    default: true,
  },
  {
    id: "canary",
    label: "Canary",
    kind: "mod",
    loaders: ["forge"],
    minMc: "1.18.2",
    maxMc: "1.20.1",
    slug: "canary",
    reason: "Port de Lithium a Forge.",
    default: true,
  },
  {
    id: "lazydfu",
    label: "LazyDFU",
    kind: "mod",
    loaders: ["fabric", "quilt", "forge"],
    minMc: "1.14",
    maxMc: "1.19.4",
    slug: "lazydfu",
    reason: "Diferir inicialización de DFU: arranque más rápido (1.19 y antes).",
    default: true,
  },
];

export const perfItemsFor = (loader: LoaderKind, mc: string): PerfItem[] => {
  const v = versionKey(parseMc(mc));
  return PERF_ITEMS.filter((it) => {
    if (!it.loaders.includes(loader)) return false;
    const min = versionKey(parseMc(it.minMc));
    const max = it.maxMc === "999" ? Infinity : versionKey(parseMc(it.maxMc));
    return v >= min && v <= max;
  });
};

/** [mayor, menor, parche] con 0 por defecto */
export function parseMc(v: string): [number, number, number] {
  const m = /^(\d+)\.(\d+)(?:\.(\d+))?/.exec(v);
  if (!m) return [0, 0, 0];
  return [Number(m[1]), Number(m[2]), Number(m[3] ?? 0)];
}

export function versionKey(v: [number, number, number]): number {
  return v[0] * 1_000_000 + v[1] * 1_000 + v[2];
}

/** Versión de Java necesaria para una versión de Minecraft */
export function javaMajorFor(mc: string): 8 | 17 | 21 | 25 {
  const [maj, min, patch] = parseMc(mc);
  const key = versionKey([maj, min, patch]);
  // MC 26.x (versionado por fecha desde 2026) exige Java 25
  if (key >= versionKey([26, 0, 0])) return 25;
  // snapshots y futuros van con la más moderna conocida
  if (maj > 1) return 25;
  if (key >= versionKey([1, 20, 5])) return 21;
  if (key >= versionKey([1, 17, 0])) return 17;
  return 8;
}
