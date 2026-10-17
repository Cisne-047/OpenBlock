import { randomBytes } from "node:crypto";
import { existsSync } from "node:fs";
import { basename, resolve } from "node:path";
import { ENGINES, engineById } from "./catalog.js";
import { applyConfig, ensureRconPassword, extractCfOverrides } from "./apply.js";
import { detectHardware } from "./hw/detect.js";
import { classifyHardware, recommendedRamMB } from "./hw/tiers.js";
import { latestRelease } from "./api/mojang.js";
import {
  fabricLoaderVersions,
  forgeBuilds,
  neoforgeBuilds,
  paperBuilds,
  purpurBuilds,
  quiltLoaderVersions,
  type BuildInfo,
} from "./api/engines.js";
import { fetchMrpack } from "./plan.js";
import { parseCfManifest, cfLoaderKind } from "./api/modrinth.js";
import { runDoctor, printReport } from "./doctor.js";
import type { CustomItem, EngineId, ServerConfig } from "./types.js";
import { ask, askChoice, askNumber, c, confirm, kvTable } from "./ui.js";

/** lista de builds disponibles según engine, para elegir la exacta */
async function buildsFor(engine: EngineId, mc: string): Promise<BuildInfo[]> {
  switch (engine) {
    case "paper":
    case "folia":
      return paperBuilds(engine, mc);
    case "purpur":
      return purpurBuilds(mc);
    case "fabric":
      return fabricLoaderVersions(mc);
    case "quilt":
      return quiltLoaderVersions(mc);
    case "forge":
      return forgeBuilds(mc);
    case "neoforge":
      return neoforgeBuilds(mc);
    default:
      return [];
  }
}

/** pregunta de build exacta: Enter = latest, número = build de la lista */
async function pickBuild(engine: EngineId, mc: string): Promise<{ build?: string; loaderBuild?: string; installerBuild?: string }> {
  if (engine === "vanilla") return {};
  console.log(c.gray("  Consultando builds disponibles..."));
  const builds = await buildsFor(engine, mc);
  if (builds.length === 0) return {};
  const top = builds.slice(0, 12);
  console.log();
  top.forEach((b, i) => {
    console.log(`  ${c.green(String(i + 1).padStart(2))}. build ${c.bold(b.id)}${b.channel ? c.gray(` (${b.channel})`) : ""}`);
  });
  console.log(c.gray(`  (${builds.length} builds en total)`));
  const raw = await ask("Build exacta del engine (número, vacío = última)", "");
  const idx = Number(raw) - 1;
  if (!raw || Number.isNaN(idx) || !top[idx]) return {};
  const chosen = top[idx].id;
  if (engine === "fabric" || engine === "quilt") return { loaderBuild: chosen };
  return { build: chosen };
}

/**
 * Modo guiado: detecta hardware, pregunta lo mínimo con defaults sanos,
 * muestra el plan y lo aplica. El server queda instalado y arrancable.
 */
export async function guided(): Promise<void> {
  const hw = await detectHardware();
  const tier = classifyHardware(hw);

  console.log(c.bold("\n== Tu hardware =="));
  kvTable([
    ["CPU", `${hw.cpuModel} (${hw.cores} hilos)`],
    ["RAM", `${(hw.ramTotalMB / 1024).toFixed(1)} GB (libres ${(hw.ramFreeMB / 1024).toFixed(1)} GB)`],
    ["Disco", hw.diskKind === "unknown" ? "desconocido" : hw.diskKind.toUpperCase()],
    ["Sistema", `${hw.os} ${hw.osVersion}${hw.virtualized ? c.yellow(" (VM)") : ""}`],
    ["Tier", c.cyan(`${tier.label} — ${tier.maxPlayers} jugadores, vd ${tier.viewDistance}/${tier.simulationDistance}`)],
  ]);
  console.log(c.dim("\n  Tip: `openblock bench` mide CPU/disco de verdad y afina la recomendación."));

  const dedicated = await askChoice(
    "\n¿Para qué es esta máquina?",
    [
      { id: "dedicada", label: "Solo el server (dedicada)", hint: "usa hasta el 70-80% de la RAM" },
      { id: "compartida", label: "La uso para más cosas", hint: "usa ~45% de la RAM" },
    ],
    "dedicada"
  );

  const mode = await askChoice(
    "¿Qué server quieres montar?",
    [
      { id: "facil", label: "Súper fácil", hint: "recomendado por tu hardware, todo automático" },
      { id: "plugins", label: "Paper con plugins de rendimiento", hint: "el estándar para jugar con amigos" },
      { id: "modpack", label: "Modpack", hint: "URL de Modrinth o zip exportado de CurseForge" },
      { id: "modded", label: "Modded a mano (Fabric/Forge/NeoForge)" },
      { id: "vanilla", label: "Vanilla puro", hint: c.yellow("sin optimizaciones, tal cual Mojang") },
    ],
    "facil"
  );

  let engine: EngineId;
  let mcVersion: string;
  let modpack: ServerConfig["modpack"] = { kind: "none", url: "" };
  let cfZipPath = "";
  let buildPick: { build?: string; loaderBuild?: string; installerBuild?: string } = {};

  if (mode === "facil") {
    engine = tier.recommendedEngine;
    mcVersion = await latestRelease();
    console.log(c.dim(`  → ${engineById(engine).label} ${mcVersion}: lo óptimo para tu máquina.`));
  } else if (mode === "vanilla") {
    engine = "vanilla";
    mcVersion = await ask("Versión de MC", await latestRelease());
  } else if (mode === "plugins") {
    engine = await askChoice<EngineId>(
      "Engine",
      ENGINES.filter((e) => e.pluginApi).map((e) => ({ id: e.id, label: e.label, hint: e.desc })),
      tier.recommendedEngine
    );
    mcVersion = await ask("Versión de MC", await latestRelease());
    buildPick = await pickBuild(engine, mcVersion);
  } else if (mode === "modded") {
    engine = await askChoice<EngineId>(
      "Loader",
      ENGINES.filter((e) => e.modApi).map((e) => ({ id: e.id, label: e.label, hint: e.desc })),
      "fabric"
    );
    mcVersion = await ask("Versión de MC", await latestRelease());
    buildPick = await pickBuild(engine, mcVersion);
  } else {
    // modpack
    const source = await askChoice<"modrinth" | "curseforge">(
      "Origen del modpack",
      [
        { id: "modrinth", label: "Modrinth (URL)", hint: "descarga automática completa" },
        { id: "curseforge", label: "CurseForge (zip exportado)", hint: "mods necesitan API key; overrides sí se copian" },
      ],
      "modrinth"
    );
    if (source === "modrinth") {
      const url = await ask("URL del modpack (modrinth.com/modpack/...)", "");
      console.log(c.dim("  Resolviendo modpack..."));
      const { index } = await fetchMrpack(url);
      const deps = index.dependencies;
      mcVersion = deps.minecraft;
      const loaderDep = Object.keys(deps).find((k) => k !== "minecraft");
      engine =
        loaderDep === "fabric-loader" ? "fabric" : loaderDep === "quilt-loader" ? "quilt" : loaderDep === "neoforge" ? "neoforge" : "forge";
      modpack = { kind: "modrinth", url };
      console.log(c.green(`  → ${index.name} ${index.versionId}: MC ${mcVersion}, loader ${loaderDep ?? "?"}`));
    } else {
      cfZipPath = await ask("Ruta del zip exportado de CurseForge", "");
      if (!existsSync(cfZipPath)) throw new Error(`No existe ${cfZipPath}`);
      const JSZip = (await import("jszip")).default;
      const zip = await JSZip.loadAsync(await (await import("node:fs/promises")).readFile(cfZipPath));
      const manifest = parseCfManifest(await zip.file("manifest.json")!.async("string"));
      const loader = cfLoaderKind(manifest);
      mcVersion = manifest.minecraft.version;
      engine = loader ? loader.engine : "forge";
      modpack = { kind: "curseforge", url: cfZipPath };
      console.log(c.green(`  → ${manifest.name}: MC ${mcVersion}, loader ${loader ? loader.engine : "?"} ${loader?.build ?? ""}`));
    }
  }

  const defaultRam = recommendedRamMB(hw, dedicated === "dedicada");
  const ramMB = await askNumber(`RAM para el server (MB)`, defaultRam);
  const players = await askNumber("Jugadores", tier.maxPlayers);
  const name = await ask("Nombre del server", `mi-server`);
  const port = await askNumber("Puerto", 25565);
  const motd = await ask("MOTD", `${engineById(engine).label} ${mcVersion} — forjado con OpenBlock`);
  const pregenRadius = await askNumber(
    "Pregenerar mundo (radio en bloques, 0 = no)",
    tier.pregenRadius || 2000
  );
  const performance = await askChoice<"conservative" | "balanced" | "aggressive">(
    "Perfil de rendimiento",
    [
      { id: "conservative", label: "Conservador", hint: "toca lo mínimo" },
      { id: "balanced", label: "Equilibrado", hint: "lo seguro, sin tocar gameplay" },
      { id: "aggressive", label: "Agresivo", hint: "más TPS, rangos de mobs reducidos" },
    ],
    "balanced"
  );
  const serviceEnabled = await confirm("¿Autoarranque como servicio del sistema?", false);
  const backupEnabled = await confirm("¿Backups automáticos (diarios, 7 copias)?", true);

  // custom items: plugins/mods propios por URL o ruta local
  const customItems: CustomItem[] = [];
  while (await confirm("¿Añadir un plugin/mod propio (URL o ruta local)?", false)) {
    const kind = await askChoice<"plugin" | "mod">(
      "¿Qué es?",
      [
        { id: "plugin", label: "Plugin (plugins/, engines Paper-like)" },
        { id: "mod", label: "Mod (mods/, loaders de mods)" },
      ],
      "plugin"
    );
    const src = await ask("URL directa del .jar o ruta local", "");
    if (!src) continue;
    const name = basename(src).replace(/\.jar$/i, "") || "custom";
    customItems.push(
      /^https?:\/\//i.test(src)
        ? { name, kind, url: src }
        : { name, kind, path: resolve(src) }
    );
    console.log(c.green(`  ✓ ${name} añadido (${kind})`));
  }

  const serverDir = await ask(
    "Directorio del server",
    process.platform === "win32" ? `F:\\Servers\\${name}` : `~/servers/${name}`
  );

  const cfg: ServerConfig = {
    name,
    mcVersion,
    engine,
    serverDir: resolve(serverDir.replace(/^~(?=\/|$)/, process.env.HOME ?? "")),
    ramMB,
    players,
    viewDistance: tier.viewDistance,
    simulationDistance: tier.simulationDistance,
    port,
    motd,
    difficulty: "normal",
    gamemode: "survival",
    worldType: "normal",
    seed: "",
    onlineMode: true,
    whitelist: false,
    pvp: true,
    hardcore: false,
    modpack,
    performance,
    pregenRadius,
    backup: { enabled: backupEnabled, intervalHours: 24, keep: 7 },
    service: { enabled: serviceEnabled },
    rcon: { enabled: pregenRadius > 0, password: randomBytes(8).toString("hex"), port: 25575 },
    chunkyborder: false,
    targetOS: process.platform === "win32" ? "windows" : "linux",
    ...buildPick,
    customItems,
  };
  ensureRconPassword(cfg);

  console.log(c.bold("\n== Config final =="));
  kvTable([
    ["Engine", `${engineById(cfg.engine).label} ${cfg.mcVersion}${cfg.build ? ` (build ${cfg.build})` : ""}${cfg.loaderBuild ? ` (loader ${cfg.loaderBuild})` : ""}`],
    ["RAM", `${cfg.ramMB} MB`],
    ["Jugadores", String(cfg.players)],
    ["Distancias", `view ${cfg.viewDistance} / sim ${cfg.simulationDistance}`],
    ["Perfil", cfg.performance],
    ["Pregen", cfg.pregenRadius ? `${cfg.pregenRadius} bloques (Chunky + RCON)` : "off"],
    ["Custom", customItems.length ? customItems.map((i) => i.name).join(", ") : "nada"],
    ["Servicio", cfg.service.enabled ? "sí" : "no"],
    ["Backups", cfg.backup.enabled ? `cada ${cfg.backup.intervalHours}h, ${cfg.backup.keep} copias` : "no"],
    ["Directorio", cfg.serverDir],
  ]);

  if (!(await confirm(c.bold("\n¿Instalamos? (baja el engine, plugins/mods y deja el server listo)")))) {
    console.log(c.yellow("Cancelado. Tu config queda en la pregunta anterior; nada se ha tocado."));
    return;
  }

  if (cfg.modpack.kind === "curseforge" && cfZipPath) {
    // overrides primero: pueden traer configs que el server necesita
    await applyConfig(cfg, { cores: hw.cores, pregen: false });
    await extractCfOverrides(cfZipPath, cfg.serverDir, (s) => console.log(s));
    console.log(
      c.yellow(
        "\nModpack CurseForge: los mods no se bajan sin API key. Instala el modpack con el launcher y copia mods/, o exporta OPENBLOCK_CF_KEY y reintenta."
      )
    );
    return;
  }

  await applyConfig(cfg, { cores: hw.cores, pregen: true });

  // doctor: dry run de arranque con fixes (solo si el server no quedó corriendo con la pregen)
  if (cfg.pregenRadius === 0) {
    const runDoc = await confirm(
      c.gold("\n¿Ejecutamos el doctor? (dry run: busca errores/warnings, arregla lo seguro, pregunta lo gordo)"),
      true
    );
    if (runDoc) {
      const report = await runDoctor(cfg, { boot: true, interactive: true });
      printReport(report);
    }
  }
}
