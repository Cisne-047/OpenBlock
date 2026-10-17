import { parseArgs } from "node:util";
import { existsSync, readFileSync } from "node:fs";
import { basename, isAbsolute, join, resolve } from "node:path";
import { latestRelease } from "./api/mojang.js";
import { applyConfig, ensureRconPassword, runBackup, stopServer } from "./apply.js";
import { detectHardware } from "./hw/detect.js";
import { classifyHardware, recommendedRamMB } from "./hw/tiers.js";
import { HELP } from "./help.js";
import type { EngineId, PerfProfile, ServerConfig } from "./types.js";
import { c, confirm, kvTable } from "./ui.js";

/**
 * Modo avanzado: flags completas + config JSON reimportable. Todo lo del modo
 * guiado es scriptable aquí, con overrides sobre los defaults de hardware.
 */
export interface AdvancedArgs {
  config?: string;
  name?: string;
  mc?: string;
  engine?: EngineId;
  ramMb?: number;
  players?: number;
  dir?: string;
  port?: number;
  vd?: number;
  sd?: number;
  profile?: PerfProfile;
  pregen?: number;
  modpack?: string;
  /** build exacta del engine (paper/purpur/folia = nº build; forge/neoforge = build loader) */
  build?: string;
  /** build exacta del loader fabric/quilt */
  loaderBuild?: string;
  /** versión del installer fabric/quilt */
  installerBuild?: string;
  /** plugins/mods custom: URLs o rutas locales, separadas por coma */
  custom?: string[];
  noService?: boolean;
  noBackup?: boolean;
  noRcon?: boolean;
  target?: "linux" | "windows" | "both";
  dryRun?: boolean;
  yes?: boolean;
  force?: boolean;
  noPregen?: boolean;
  /** pide la ayuda en vez de construir la config */
  help?: boolean;
}

export function parseAdvancedArgs(argv: string[]): AdvancedArgs {
  const { values } = parseArgs({
    args: argv,
    options: {
      config: { type: "string" },
      name: { type: "string" },
      mc: { type: "string" },
      engine: { type: "string" },
      "ram-mb": { type: "string" },
      players: { type: "string" },
      dir: { type: "string" },
      port: { type: "string" },
      vd: { type: "string" },
      sd: { type: "string" },
      profile: { type: "string" },
      pregen: { type: "string" },
      modpack: { type: "string" },
      build: { type: "string" },
      "loader-build": { type: "string" },
      "installer-build": { type: "string" },
      custom: { type: "string" },
      "no-service": { type: "boolean" },
      "no-backup": { type: "boolean" },
      "no-rcon": { type: "boolean" },
      "no-pregen": { type: "boolean" },
      target: { type: "string" },
      "dry-run": { type: "boolean" },
      yes: { type: "boolean" },
      force: { type: "boolean" },
      help: { type: "boolean" },
    },
    strict: false,
  });
  return {
    config: values.config as string | undefined,
    name: values.name as string | undefined,
    mc: values.mc as string | undefined,
    engine: values.engine as EngineId | undefined,
    ramMb: values["ram-mb"] ? Number(values["ram-mb"]) : undefined,
    players: values.players ? Number(values.players) : undefined,
    dir: values.dir as string | undefined,
    port: values.port ? Number(values.port) : undefined,
    vd: values.vd ? Number(values.vd) : undefined,
    sd: values.sd ? Number(values.sd) : undefined,
    profile: values.profile as PerfProfile | undefined,
    pregen: values.pregen ? Number(values.pregen) : undefined,
    modpack: values.modpack as string | undefined,
    build: values.build as string | undefined,
    loaderBuild: values["loader-build"] as string | undefined,
    installerBuild: values["installer-build"] as string | undefined,
    custom: values.custom ? String(values.custom).split(",").map((s) => s.trim()).filter(Boolean) : undefined,
    noService: !!values["no-service"],
    noBackup: !!values["no-backup"],
    noRcon: !!values["no-rcon"],
    noPregen: !!values["no-pregen"],
    target: values.target as AdvancedArgs["target"],
    dryRun: !!values["dry-run"],
    yes: !!values.yes,
    force: !!values.force,
    help: !!values.help,
  };
}

export async function buildAdvancedConfig(a: AdvancedArgs): Promise<ServerConfig> {
  // base: config JSON previa (round-trip) o defaults de hardware
  let base: Partial<ServerConfig> = {};
  if (a.config) {
    if (!existsSync(a.config)) throw new Error(`No existe ${a.config}`);
    base = JSON.parse(readFileSync(a.config, "utf8")) as Partial<ServerConfig>;
  }
  const hw = await detectHardware();
  const tier = classifyHardware(hw);
  const mc = a.mc ?? base.mcVersion ?? (await latestRelease());
  const modpackUrl = a.modpack ?? (base.modpack && base.modpack.kind !== "none" ? base.modpack.url : "");
  const engine = a.engine ?? base.engine ?? tier.recommendedEngine;
  const name = a.name ?? base.name ?? "mi-server";

  const cfg: ServerConfig = {
    name,
    mcVersion: mc,
    engine,
    serverDir: resolve(a.dir ?? base.serverDir ?? (process.platform === "win32" ? `F:\\Servers\\${name}` : `~/servers/${name}`)),
    ramMB: a.ramMb ?? base.ramMB ?? recommendedRamMB(hw, true),
    players: a.players ?? base.players ?? tier.maxPlayers,
    viewDistance: a.vd ?? base.viewDistance ?? tier.viewDistance,
    simulationDistance: a.sd ?? base.simulationDistance ?? tier.simulationDistance,
    port: a.port ?? base.port ?? 25565,
    motd: base.motd ?? `${engine} ${mc} — forjado con OpenBlock`,
    difficulty: base.difficulty ?? "normal",
    gamemode: base.gamemode ?? "survival",
    worldType: base.worldType ?? "normal",
    seed: base.seed ?? "",
    onlineMode: base.onlineMode ?? true,
    whitelist: base.whitelist ?? false,
    pvp: base.pvp ?? true,
    hardcore: base.hardcore ?? false,
    modpack: modpackUrl ? { kind: base.modpack?.kind ?? "modrinth", url: modpackUrl } : { kind: "none", url: "" },
    performance: a.profile ?? base.performance ?? "balanced",
    pregenRadius: a.pregen ?? base.pregenRadius ?? tier.pregenRadius,
    backup: {
      enabled: !(a.noBackup ?? false) && (base.backup?.enabled ?? true),
      intervalHours: base.backup?.intervalHours ?? 24,
      keep: base.backup?.keep ?? 7,
    },
    service: { enabled: !(a.noService ?? false) && (base.service?.enabled ?? false), user: base.service?.user },
    rcon: {
      enabled: !(a.noRcon ?? false) && (base.rcon?.enabled ?? true),
      password: base.rcon?.password ?? "",
      port: base.rcon?.port ?? 25575,
    },
    chunkyborder: base.chunkyborder ?? false,
    targetOS: a.target ?? base.targetOS ?? "both",
    build: a.build ?? base.build,
    loaderBuild: a.loaderBuild ?? base.loaderBuild,
    installerBuild: a.installerBuild ?? base.installerBuild,
    customItems:
      a.custom?.map((src) => {
        const name = basename(src).replace(/\.jar$/i, "") || "custom";
        const customKind = ["fabric", "quilt", "forge", "neoforge"].includes(engine) ? ("mod" as const) : ("plugin" as const);
        return {
          name,
          kind: customKind,
          ...(/^https?:\/\//i.test(src) ? { url: src } : { path: resolve(src) }),
        };
      }) ??
      base.customItems ??
      [],
  };
  ensureRconPassword(cfg);
  return cfg;
}

export async function advanced(argv: string[]): Promise<void> {
  const a = parseAdvancedArgs(argv);
  if (a.help) {
    console.log(HELP);
    return;
  }
  const cfg = await buildAdvancedConfig(a);
  const hw = await detectHardware();
  console.log(c.bold("\n== Config avanzada =="));
  kvTable([
    ["Engine", `${cfg.engine} ${cfg.mcVersion}`],
    ["RAM", `${cfg.ramMB} MB`],
    ["Directorio", cfg.serverDir],
    ["Perfil", cfg.performance],
    ["Pregen", String(cfg.pregenRadius)],
  ]);
  if (!a.yes && !(await confirm(c.bold("¿Aplicamos?")))) {
    console.log(c.yellow("Cancelado."));
    return;
  }
  await applyConfig(cfg, { cores: hw.cores, dryRun: a.dryRun, force: a.force, pregen: !a.noPregen });
}

export async function backupCommand(dir: string): Promise<void> {
  await runBackup(dir);
}

export async function stopCommand(dir: string): Promise<void> {
  const cfgPath = resolve(isAbsolute(dir) ? dir : join(process.cwd(), dir), "openblock.json");
  if (!existsSync(cfgPath)) throw new Error(`No existe ${cfgPath}`);
  const cfg = JSON.parse(readFileSync(cfgPath, "utf8")) as ServerConfig;
  // si el serverDir embebido es de otro SO (ruta Windows desde Linux o viceversa),
  // se reemplaza por el directorio actual: el doctor/apply opera sobre --dir
  if (!existsSync(cfg.serverDir)) cfg.serverDir = cfgPath.slice(0, -"openblock.json".length);
  await stopServer(cfg);
  console.log(c.green("Stop enviado por RCON."));
}
