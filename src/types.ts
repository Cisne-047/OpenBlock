export type EngineId =
  | "vanilla"
  | "paper"
  | "purpur"
  | "folia"
  | "fabric"
  | "quilt"
  | "forge"
  | "neoforge";

export type LoaderKind = "none" | "bukkit" | "paper" | "fabric" | "quilt" | "forge" | "neoforge";

export type PerfProfile = "conservative" | "balanced" | "aggressive";

export type DiskKind = "nvme" | "ssd" | "hdd" | "unknown";

export interface BenchmarkResult {
  /** ops de CPU por core por ms, normalizado */
  cpuScore: number;
  /** MB/s de escritura secuencial */
  diskMBs: number;
  durationMs: number;
}

export interface HardwareProfile {
  os: "windows" | "linux";
  osVersion: string;
  arch: string;
  hostname: string;
  cpuModel: string;
  /** cores lógicos */
  cores: number;
  ramTotalMB: number;
  ramFreeMB: number;
  swapTotalMB: number;
  diskKind: DiskKind;
  diskFreeMB: number;
  virtualized: boolean;
  bench?: BenchmarkResult;
}

export type TierId = "potato" | "low" | "mid" | "high" | "enthusiast";

export interface Tier {
  id: TierId;
  label: string;
  maxPlayers: number;
  viewDistance: number;
  simulationDistance: number;
  pregenRadius: number;
  recommendedEngine: EngineId;
  gc: "g1" | "zgc";
  /** % de RAM total que conviene asignar si el server está dedicado */
  dedicatedRamPct: number;
}

export type ModpackKind = "none" | "modrinth" | "curseforge";

export interface ModpackRef {
  kind: ModpackKind;
  /** URL de la página del modpack o del .mrpack */
  url: string;
}

/** Plugin/mod propio del usuario: URL directa o fichero local */
export interface CustomItem {
  name: string;
  kind: "plugin" | "mod";
  /** URL http(s) de descarga directa (jar) */
  url?: string;
  /** ruta local al jar (se copia al server) */
  path?: string;
}

export interface ServerConfig {
  name: string;
  mcVersion: string;
  engine: EngineId;
  serverDir: string;
  /** -Xmx en MB */
  ramMB: number;
  players: number;
  viewDistance: number;
  simulationDistance: number;
  port: number;
  motd: string;
  difficulty: "peaceful" | "easy" | "normal" | "hard";
  gamemode: "survival" | "creative" | "adventure";
  worldType: "normal" | "flat" | "large_biomes" | "amplified";
  seed: string;
  onlineMode: boolean;
  whitelist: boolean;
  pvp: boolean;
  hardcore: boolean;
  modpack: ModpackRef;
  performance: PerfProfile;
  /** pregeneración con Chunky; 0 = off */
  pregenRadius: number;
  backup: { enabled: boolean; intervalHours: number; keep: number };
  service: { enabled: boolean; user?: string };
  rcon: { enabled: boolean; password: string; port: number };
  /** ruta java explícita; vacío = autogestionado por OpenBlock */
  javaPath?: string;
  /** chunkyborder junto a chunky */
  chunkyborder: boolean;
  /** SO objetivo de los scripts generados */
  targetOS: "linux" | "windows" | "both";
  /** build exacta del engine: paper/purpur/folia = nº de build; forge/neoforge = build del loader */
  build?: string;
  /** build exacta del loader (fabric/quilt) */
  loaderBuild?: string;
  /** versión del installer (fabric/quilt); vacío = latest */
  installerBuild?: string;
  /** plugins/mods propios del usuario */
  customItems: CustomItem[];
}
