/**
 * Genera el golden de paridad para el port a Rust.
 *
 * El núcleo en Rust (`rust/crates/forge-core`) compara su salida byte a byte
 * contra estos ficheros. Es el equivalente del `diff -r` documentado de
 * DistroForge, pero automatizado: si alguien toca un generador en TS y no en
 * Rust (o al revés), `cargo test` lo dice con el fichero y la línea exactos.
 *
 * Uso:  npm run parity:golden
 * Test: cargo test --manifest-path rust/Cargo.toml
 */
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { generateProject } from "../src/generators/index.js";
import { serverProperties } from "../src/generators/properties.js";
import { jvmArgsLine } from "../src/generators/jvm.js";
import { bukkitYml, paperGlobalYml, paperWorldDefaultsYml, spigotYml } from "../src/generators/paper.js";
import { backupPs1, backupSh, backupTimer, firewallPs1, firewallSh } from "../src/generators/ops.js";
import { installServicePs1, installServiceSh, removeServicePs1, systemdUnit, winswConfig } from "../src/generators/service.js";
import { ENGINES, PERF_ITEMS, javaMajorFor, parseMc, perfItemsFor } from "../src/catalog.js";
import { TIERS, classifyHardware, recommendedGc, recommendedRamMB } from "../src/hw/tiers.js";
import { encodePacket } from "../src/rcon.js";
import { planFor } from "./helpers.js";
import type { HardwareProfile, ServerConfig } from "../src/types.js";

const raiz = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const golden = join(raiz, "rust", "crates", "forge-core", "tests", "golden");

const cfg = JSON.parse(readFileSync(join(raiz, "tests", "fixtures", "parity-config.json"), "utf8")) as ServerConfig;
const plan = planFor(cfg);
const cores = 16;
const gc = cfg.ramMB >= 16 * 1024 && cores >= 8 ? "zgc" : "g1";
const argsLine = jvmArgsLine(cfg, gc, cores);
const user = cfg.service.user ?? "minecraft";

// Los generadores de hoja, cada uno a su fichero: así el fallo señala cuál.
const sueltos: Record<string, string> = {
  "jvm-args.txt": argsLine,
  "server.properties": serverProperties(cfg),
  "paper-global.yml": paperGlobalYml(cfg),
  "paper-world-defaults.yml": paperWorldDefaultsYml(cfg),
  "spigot.yml": spigotYml(cfg),
  "bukkit.yml": bukkitYml(cfg),
  "firewall.sh": firewallSh(cfg),
  "firewall.ps1": firewallPs1(cfg),
  "backup.sh": backupSh(cfg),
  "backup.ps1": backupPs1(cfg),
  "openblock-backup.timer": backupTimer(cfg),
  "openblock.service": systemdUnit(cfg, user),
  "install-service.sh": installServiceSh(cfg, user),
  "winsw.xml": winswConfig(cfg),
  "install-service.ps1": installServicePs1(cfg),
  "remove-service.ps1": removeServicePs1(cfg),
};

rmSync(golden, { recursive: true, force: true });
mkdirSync(join(golden, "leaves"), { recursive: true });
for (const [nombre, contenido] of Object.entries(sueltos)) {
  writeFileSync(join(golden, "leaves", nombre), contenido, "utf8");
}
// El proyecto completo (incluye los scripts, que aún no están portados).
const proyecto = generateProject(cfg, plan, cores);
mkdirSync(join(golden, "project"), { recursive: true });
for (const [nombre, contenido] of Object.entries(proyecto)) {
  writeFileSync(join(golden, "project", nombre), contenido, "utf8");
}
// El plan entra en el golden porque `generateProject` lo necesita: los scripts
// llevan sus URLs y hashes. Así Rust no tiene que adivinar de dónde salen.
writeFileSync(join(golden, "plan.json"), JSON.stringify(plan, null, 2) + "\n", "utf8");
writeFileSync(join(golden, "cores.txt"), String(cores) + "\n", "utf8");

// Catálogo: las tablas enteras, no solo el comportamiento. Aquí es donde se cuela
// una errata en una `desc` o un `reason` que ningún test de comportamiento ve.
const catalogo = {
  engines: ENGINES.map((e) => ({
    id: e.id,
    label: e.label,
    loader: e.loader,
    desc: e.desc,
    pluginApi: e.pluginApi,
    modApi: e.modApi,
    multithread: e.multithread,
    installer: e.installer,
    modpackLoader: e.modpackLoader,
  })),
  perfItems: PERF_ITEMS.map((p) => ({
    id: p.id,
    label: p.label,
    kind: p.kind,
    loaders: p.loaders,
    minMc: p.minMc,
    maxMc: p.maxMc,
    slug: p.slug,
    reason: p.reason,
    default: p.default,
  })),
  // resolución por loader×versión, que es lo que consume el plan
  perfPorLoader: Object.fromEntries(
    ["none", "paper", "fabric", "quilt", "forge", "neoforge"].map((l) => [
      l,
      ["1.16.5", "1.18.2", "1.19.4", "1.20.1", "1.20.4", "1.21.4"].map((mc) => ({
        mc,
        ids: perfItemsFor(l as never, mc).map((i) => i.id),
      })),
    ])
  ),
  // mapa Java/MC, incluidos los bordes
  javaPorMc: Object.fromEntries(
    ["1.12.2", "1.16.5", "1.17", "1.17.1", "1.18.2", "1.20.4", "1.20.5", "1.20.6", "1.21.4", "26.1", "26.3", "26w14a"].map((mc) => [
      mc,
      javaMajorFor(mc),
    ])
  ),
  parseMc: Object.fromEntries(
    [
      "1.21.4",
      "1.20",
      "26.3",
      "26w14a",
      "1.2b.3",
      "1.20.b",
      "1.20.4-rc1",
      "1.",
      ".5",
      "1.2.",
      "1.2.3.4",
      "10.20.30",
      "basura",
      "",
    ].map((v) => [v, parseMc(v)])
  ),
};
writeFileSync(join(golden, "catalog.json"), JSON.stringify(catalogo, null, 2) + "\n", "utf8");

// Tiers: la tabla y la clasificación de perfiles variados. Los casos límite son
// donde una resta o un redondeo distinto cambia el tier, así que se cubren RAM
// justa, cores justos, disco lento, VM y benchmark.
const hwBase: HardwareProfile = {
  os: "linux",
  osVersion: "6.8",
  arch: "x64",
  hostname: "test",
  cpuModel: "Test CPU",
  cores: 8,
  ramTotalMB: 16384,
  ramFreeMB: 8192,
  swapTotalMB: 0,
  diskKind: "nvme",
  diskFreeMB: 50000,
  virtualized: false,
};

const perfiles: Array<[string, Partial<HardwareProfile>]> = [
  ["base nvme 8c/16GB", {}],
  ["ram justa 2GB", { ramTotalMB: 2048 }],
  ["ram 4GB", { ramTotalMB: 4096 }],
  ["un solo core", { cores: 1 }],
  ["dos cores", { cores: 2 }],
  ["hdd 12c/32GB", { diskKind: "hdd", ramTotalMB: 32768, cores: 12 }],
  ["ssd 8c/16GB", { diskKind: "ssd" }],
  ["disco desconocido", { diskKind: "unknown" }],
  ["vm penalizada", { virtualized: true }],
  ["vm con pepino", { virtualized: true, cores: 24, ramTotalMB: 65536 }],
  ["pepino 24c/64GB", { cores: 24, ramTotalMB: 65536 }],
  ["sin bench vs bench alto", { diskKind: "hdd", bench: { cpuScore: 9000, diskMBs: 500, durationMs: 1000 } }],
  ["bench justo en el borde", { cores: 8, ramTotalMB: 16384, bench: { cpuScore: 7000, diskMBs: 400, durationMs: 900 } }],
  ["bench a cero", { bench: { cpuScore: 0, diskMBs: 0, durationMs: 0 } }],
  ["32GB compartida vs dedicada", { ramTotalMB: 32768 }],
  ["64GB", { ramTotalMB: 65536, cores: 16 }],
  ["128GB muchos cores", { ramTotalMB: 131072, cores: 32 }],
  ["1GB de ram", { ramTotalMB: 1024 }],
  ["3 cores", { cores: 3 }],
  ["7 cores (borde de zgc)", { cores: 7, ramTotalMB: 16384 }],
  ["8 cores con 15GB (borde de zgc)", { cores: 8, ramTotalMB: 15360 }],
];

const tiersGolden = {
  tiers: Object.values(TIERS).map((t) => ({ ...t })),
  casos: perfiles.map(([nombre, over]) => {
    const h: HardwareProfile = { ...hwBase, ...over };
    return {
      nombre,
      hw: h,
      tier: classifyHardware(h).id,
      ramDedicada: recommendedRamMB(h, true),
      ramCompartida: recommendedRamMB(h, false),
      gc: recommendedGc(h),
    };
  }),
};
writeFileSync(join(golden, "tiers.json"), JSON.stringify(tiersGolden, null, 2) + "\n", "utf8");

// RCON: los bytes exactos que salen del códec. Un byte mal puesto en un protocolo
// binario no da un error legible, da un server que no responde.
const casosRcon: Array<[number, number, string]> = [
  [7, 3, "pass123"],
  [1, 2, "chunky radius 4000"],
  [2, 0, ""],
  [1, 2, "spark tps"],
  [0, 0, "a"],
  [2147483647, 2, "id grande"],
  [-1, 2, "id negativo"],
  [1, 2, "acentué ñ"],
  [1, 2, "con\nsalto"],
];
const rconGolden = {
  casos: casosRcon.map(([id, tipo, payload]) => ({
    id,
    tipo,
    payload,
    // hex del paquete completo, que es lo comparable byte a byte
    hex: encodePacket(id, tipo, payload).toString("hex"),
  })),
};
writeFileSync(join(golden, "rcon.json"), JSON.stringify(rconGolden, null, 2) + "\n", "utf8");

console.log(
  `golden: ${Object.keys(sueltos).length} generadores de hoja + ${Object.keys(proyecto).length} ficheros de proyecto + catálogo + ${tiersGolden.casos.length} perfiles + ${rconGolden.casos.length} paquetes RCON`
);
console.log(`en ${golden}`);
