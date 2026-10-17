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
import { cfLoaderKind, mrpackServerFiles, parseCfManifest, parseMrpackIndex } from "../src/api/modrinth.js";
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

// Benchmark: la fórmula del score y el algoritmo puro. Los números crudos de un
// benchmark no son reproducibles ni entre dos ejecuciones de la misma
// implementación, así que lo comparable es la aritmética y el xorshift.
//
// La fórmula replica `workerScore` de `src/hw/bench.ts:45`:
//   Math.round(Number(total / BigInt(cores)) / elapsed)
// y `diskWriteTest` de `:67`:
//   Math.max(1, Math.round(TOTAL / 1048576 / secs))
// Se repite aquí en vez de exportarla porque no merece una API pública en el
// fuente; el test de paridad del lado Rust es quien la vigila.
const scoreCpu = (total: bigint, cores: number, elapsed: number): number =>
  Math.round(Number(total / BigInt(Math.max(1, cores))) / Math.max(1, elapsed));
const scoreDisco = (bytes: number, secs: number): number => Math.max(1, Math.round(bytes / 1048576 / secs));

/** Un paso del xorshift de `WORKER_SRC` (src/hw/bench.ts:21-22). */
function xorshift(x: bigint): bigint {
  x ^= x << 13n;
  x ^= x >> 7n;
  x ^= x << 17n;
  if ((x & 1n) === 1n) x += 0x9e3779b97f4a7c15n;
  return x;
}
const xorshiftN = (semilla: bigint, pasos: number): string => {
  let x = semilla;
  for (let i = 0; i < pasos; i++) x = xorshift(x);
  return x.toString();
};

const benchGolden = {
  // (totalOps, cores, elapsedMs) -> score
  cpuScore: (
    [
      [72000n, 8, 1800],
      [7000n, 1, 1],
      [1n, 1, 1],
      [0n, 1, 1],
      [123456789n, 16, 1801],
      [100n, 0, 100],
      [100n, 4, 0],
      [999999999999n, 32, 1800],
      [18446744073709551615n, 8, 1800],
    ] as Array<[bigint, number, number]>
  ).map(([total, cores, elapsed]) => ({
    totalOps: total.toString(),
    cores,
    elapsedMs: elapsed,
    score: scoreCpu(total, cores, elapsed),
  })),
  diskMBs: (
    [
      [134217728, 1],
      [134217728, 2],
      [134217728, 0.5],
      [134217728, 1000],
      [0, 1],
    ] as Array<[number, number]>
  ).map(([bytes, secs]) => ({ bytes, secs, mbs: scoreDisco(bytes, secs) })),
  // el estado final del xorshift tras N pasos: si el algoritmo difiere, esto lo ve
  xorshift: [
    { semilla: "3", pasos: 1 },
    { semilla: "3", pasos: 1000 },
    { semilla: "1", pasos: 1 },
    { semilla: "12345678901234567", pasos: 5000 },
    { semilla: "18446744073709551615", pasos: 777 },
  ].map(({ semilla, pasos }) => ({ semilla, pasos, final: xorshiftN(BigInt(semilla), pasos) })),
};
writeFileSync(join(golden, "bench.json"), JSON.stringify(benchGolden, null, 2) + "\n", "utf8");

// Parsers de modpack: los mismos índices que prueba `tests/protocol.test.ts`, más
// bordes que el TS no cubre. Se guardan las ENTRADAS (el JSON) y las SALIDAS que
// produce el TS, para que Rust haga lo mismo con lo mismo.
const MRPACK_VALIDO = JSON.stringify({
  formatVersion: 1,
  game: "minecraft",
  versionId: "1.0.0",
  name: "Pack de prueba",
  files: [
    { path: "mods/lithium.jar", env: { client: "required", server: "required" }, downloads: ["https://cdn.modrinth.com/lithium.jar"], hashes: { sha1: "aa", sha512: "bb" }, fileSize: 100 },
    { path: "mods/sodium-client.jar", env: { client: "required", server: "unsupported" }, downloads: ["https://cdn.modrinth.com/sodium.jar"], hashes: {}, fileSize: 100 },
    { path: "config/optimizado.toml", downloads: ["https://cdn.modrinth.com/cfg.toml"], hashes: {}, fileSize: 10 },
    { path: "mods/sin-env.jar", downloads: ["https://cdn.modrinth.com/sinenv.jar"], hashes: {}, fileSize: 5 },
    { path: "mods/cliente-only.jar", env: { client: "required", server: "unsupported" }, downloads: [], hashes: {}, fileSize: 1 },
  ],
  dependencies: { minecraft: "1.21.1", "fabric-loader": "0.16.9" },
});

const CF_VALIDO = JSON.stringify({
  manifestType: "minecraftModpack",
  manifestVersion: 1,
  name: "Pack CF",
  minecraft: { version: "1.20.1", modLoaders: [{ id: "forge-47.2.0", primary: true }] },
  files: [{ projectID: 1, fileID: 2, required: true }],
  overrides: "overrides",
});

const modpackGolden = {
  mrpack: [
    { nombre: "válido del TS", json: MRPACK_VALIDO },
    { nombre: "formatVersion 2", json: JSON.stringify({ formatVersion: 2, versionId: "x" }) },
    { nombre: "formatVersion ausente", json: JSON.stringify({ versionId: "x" }) },
    { nombre: "json roto", json: "{ no es json }" },
    { nombre: "sin ficheros", json: JSON.stringify({ formatVersion: 1, versionId: "x" }) },
  ].map(({ nombre, json }) => {
    // Las dos llamadas se registran por separado: en el TS, `parseMrpackIndex`
    // puede pasar y `mrpackServerFiles` reventar después (si falta `files`),
    // porque no comprueba nada. Mezclarlas ocultaría esa diferencia.
    //
    // Y **no** se guarda el mensaje de error: el de `JSON.parse` cambia entre
    // versiones de Node ("...at position 2" vs "...at position 2 (line 1 column
    // 3)"), así que meterlo aquí haría el golden irreproducible —lo cazó la
    // compuerta de "el golden está al día"— y Rust nunca lo va a reproducir de
    // todas formas. Se guarda que falló, que es lo comparable.
    try {
      const idx = parseMrpackIndex(json);
      try {
        return { nombre, parseOk: true, serverOk: true, formato: idx.formatVersion, dependencias: idx.dependencies, servidor: mrpackServerFiles(idx).map((f) => f.path) };
      } catch {
        return { nombre, parseOk: true, serverOk: false, formato: idx.formatVersion, dependencias: idx.dependencies, servidor: [] as string[] };
      }
    } catch {
      return { nombre, parseOk: false, serverOk: false, formato: 0, dependencias: {}, servidor: [] as string[] };
    }
  }),
  curseforge: [
    { nombre: "forge primario", json: CF_VALIDO },
    { nombre: "neoforge primario", json: JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "neoforge-21.1.0", primary: true }] } }) },
    { nombre: "fabric (no es forge)", json: JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "fabric-0.16.9", primary: true }] } }) },
    { nombre: "sin primario", json: JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "forge-1.0", primary: false }] } }) },
    { nombre: "sin loaders", json: JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [] } }) },
    { nombre: "forge con build vacío", json: JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "forge-", primary: true }] } }) },
  ].map(({ nombre, json }) => {
    try {
      const m = parseCfManifest(json);
      const k = cfLoaderKind(m);
      return { nombre, ok: true, engine: k?.engine ?? null, build: k?.build ?? null, mc: m.minecraft.version, ficheros: m.files.length };
    } catch (e) {
      return { nombre, ok: false, engine: null, build: null, mc: "", ficheros: 0 };
    }
  }),
  // un manifest que no es de modpack: el TS lo rechaza con mensaje propio
  noModpack: [
    { nombre: "manifestType otro", json: JSON.stringify({ manifestType: "otro", minecraft: { version: "1", modLoaders: [] } }) },
    { nombre: "sin manifestType", json: JSON.stringify({ minecraft: { version: "1", modLoaders: [] } }) },
  ].map(({ nombre, json }) => {
    try {
      parseCfManifest(json);
      return { nombre, ok: true, error: null as string | null };
    } catch (e) {
      return { nombre, ok: false };
    }
  }),
};
writeFileSync(join(golden, "modpack.json"), JSON.stringify(modpackGolden, null, 2) + "\n", "utf8");

// Las entradas, por nombre, en un fichero aparte: Rust las necesita para repetir
// el caso y así no hay que duplicar los JSON a mano (que es donde se cuela una
// diferencia entre lo que se prueba y lo que se dijo que se probó).
const entradas: Record<string, string> = {
  "mrpack-valido": MRPACK_VALIDO,
  "mrpack-formato-2": JSON.stringify({ formatVersion: 2, versionId: "x" }),
  "mrpack-sin-formato": JSON.stringify({ versionId: "x" }),
  "mrpack-sin-ficheros": JSON.stringify({ formatVersion: 1, versionId: "x" }),
  "forge primario": CF_VALIDO,
  "neoforge primario": JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "neoforge-21.1.0", primary: true }] } }),
  "fabric (no es forge)": JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "fabric-0.16.9", primary: true }] } }),
  "sin primario": JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "forge-1.0", primary: false }] } }),
  "sin loaders": JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [] } }),
  "forge con build vacío": JSON.stringify({ ...JSON.parse(CF_VALIDO), minecraft: { version: "1.21", modLoaders: [{ id: "forge-", primary: true }] } }),
  "manifestType otro": JSON.stringify({ manifestType: "otro", minecraft: { version: "1", modLoaders: [] } }),
  "sin manifestType": JSON.stringify({ minecraft: { version: "1", modLoaders: [] } }),
};
writeFileSync(join(golden, "modpack-entradas.json"), JSON.stringify(entradas, null, 2) + "\n", "utf8");

console.log(
  `golden: ${Object.keys(sueltos).length} generadores de hoja + ${Object.keys(proyecto).length} ficheros de proyecto + catálogo + ${tiersGolden.casos.length} perfiles + ${rconGolden.casos.length} paquetes RCON + ${benchGolden.cpuScore.length} scores + ${modpackGolden.mrpack.length + modpackGolden.curseforge.length + modpackGolden.noModpack.length} casos de modpack`
);
console.log(`en ${golden}`);
