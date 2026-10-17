import type { DownloadPlan } from "../src/plan.js";
import type { ServerConfig } from "../src/types.js";

/**
 * Factoría de configs para los tests.
 *
 * DistroForge no usa ficheros de fixture ni snapshots: construye los datos con
 * helpers pequeños (`cfg_for`, `app_cfg`, `repo` en `tests/generators.rs:35-59`).
 * Aquí el literal de `ServerConfig` estaba copiado entero en dos suites, así que
 * se centraliza: un campo nuevo se añade en un solo sitio.
 */

export const serverCfg = (over: Partial<ServerConfig> = {}): ServerConfig => ({
  name: "mi-server",
  mcVersion: "1.21.4",
  engine: "paper",
  serverDir: "/tmp/server",
  ramMB: 8192,
  players: 20,
  viewDistance: 8,
  simulationDistance: 6,
  port: 25565,
  motd: "Hola mundo",
  difficulty: "normal",
  gamemode: "survival",
  worldType: "normal",
  seed: "",
  onlineMode: true,
  whitelist: false,
  pvp: true,
  hardcore: false,
  modpack: { kind: "none", url: "" },
  performance: "balanced",
  pregenRadius: 4000,
  backup: { enabled: true, intervalHours: 24, keep: 7 },
  service: { enabled: true, user: "minecraft" },
  rcon: { enabled: true, password: "secreto1234", port: 25575 },
  chunkyborder: false,
  targetOS: "both",
  customItems: [],
  ...over,
});

/** Config mínima para el doctor: sin servicio ni backups, que no son su objeto. */
export const doctorCfg = (over: Partial<ServerConfig> = {}): ServerConfig =>
  serverCfg({
    name: "test",
    motd: "test",
    pregenRadius: 0,
    backup: { enabled: false, intervalHours: 24, keep: 7 },
    service: { enabled: false },
    ...over,
  });

/** Manifiesto de descargas de ejemplo, alineado con el engine de la config. */
export const planFor = (cfg: ServerConfig = serverCfg()): DownloadPlan => {
  const forge = cfg.engine === "forge" || cfg.engine === "neoforge";
  const vanilla = cfg.engine === "vanilla";
  const filename = forge ? "forge-installer.jar" : vanilla ? "server.jar" : `paper-${cfg.mcVersion}-100.jar`;
  const label = forge ? "Forge" : vanilla ? "Vanilla" : "Paper";
  return {
    entries: [
      { kind: "engine", label, url: `https://example/${filename}`, dest: filename },
      { kind: "plugin", label: "spark", url: "https://example/spark.jar", dest: "plugins/spark.jar" },
    ],
    // forge/neoforge: el installer se ejecuta antes del primer arranque y lee user_jvm_args.txt
    ...(forge ? { installer: { jar: filename, engine: cfg.engine as "forge" | "neoforge" } } : {}),
    engineFilename: filename,
    engineResolved: forge || vanilla ? cfg.mcVersion : "build 100",
    localCopies: [],
  };
};

/** Contexto del doctor para probar reglas sin arrancar un server. */
export const ctxFor = (over: Partial<ServerConfig> = {}) => {
  const cfg = doctorCfg(over);
  return { cfg, dir: cfg.serverDir, logLines: [] as string[], bootTested: false, bootOk: false };
};
