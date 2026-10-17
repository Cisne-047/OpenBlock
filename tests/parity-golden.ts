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
import {
  fabricDownloadFrom,
  fabricInstallerVersionsFrom,
  fabricLoaderVersionsFrom,
  forgeBuildsFrom,
  forgeDownloadFrom,
  forgePickFrom,
  latestVersionFrom,
  neoforgeBuildsFrom,
  neoforgeDownloadFrom,
  neoforgePrefix,
  neoforgeVersionsFrom,
  paperBuildsFrom,
  papermcV2From,
  papermcV3From,
  purpurBuildsFrom,
  purpurDownloadFrom,
  quiltDownloadFrom,
  quiltInstallerVersionsFrom,
  quiltLoaderVersionsFrom,
} from "../src/api/engines.js";
import { planFor } from "./helpers.js";
import type { HardwareProfile, ServerConfig } from "../src/types.js";

const raiz = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const golden = join(raiz, "rust", "crates", "forge-core", "tests", "golden");
const goldenApi = join(raiz, "rust", "crates", "forge-api", "tests", "golden");

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

// Doctor: la aritmética del score. Es lo que un puerto puede romper en silencio —
// un score mal calculado no da error, da un informe que miente sobre el estado del
// server—, así que se compara con las combinaciones que tocan los cortes de nota.
//
// La fórmula replica `runDoctor` de `src/doctor.ts:601-604`:
//   Math.max(0, 100 - errors*20 - warnings*5)
//   score >= 90 ? "SANO" : >= 70 ? "OK CON AVISOS" : >= 40 ? "NECESITA ATENCIÓN" : "CRÍTICO"
// Se repite aquí en vez de exportarla porque no merece una API pública en el fuente.
const scoreDoctor = (errors: number, warnings: number, infos: number) => {
  const score = Math.max(0, 100 - errors * 20 - warnings * 5);
  const grade = score >= 90 ? "SANO" : score >= 70 ? "OK CON AVISOS" : score >= 40 ? "NECESITA ATENCIÓN" : "CRÍTICO";
  return { errors, warnings, infos, score, grade };
};

const doctorGolden = {
  // todas las combinaciones que tocan los cortes de nota y sus alrededores
  scores: (() => {
    const casos: Array<{ errors: number; warnings: number; infos: number }> = [];
    for (let errors = 0; errors <= 6; errors++) {
      for (let warnings = 0; warnings <= 6; warnings++) {
        casos.push({ errors, warnings, infos: errors + warnings });
      }
    }
    // y los bordes exactos de los cortes, con solo warnings
    for (const w of [0, 1, 2, 3, 5, 6, 10, 12, 20]) casos.push({ errors: 0, warnings: w, infos: 0 });
    return casos.map((c) => scoreDoctor(c.errors, c.warnings, c.infos));
  })(),
  // dedupe: gana el primero por código y se conserva el orden
  dedupe: (() => {
    const conDuplicados = [
      { severity: "error", code: "EULA_FILE" },
      { severity: "warning", code: "SYNC_CHUNK_WRITES" },
      { severity: "error", code: "EULA_FILE" },
      { severity: "info", code: "PLUGIN_UPDATE" },
      { severity: "warning", code: "SYNC_CHUNK_WRITES" },
      { severity: "error", code: "ENGINE_JAR_MISSING" },
    ];
    const vistos = new Set<string>();
    const unicos = conDuplicados.filter((f) => (vistos.has(f.code) ? false : (vistos.add(f.code), true)));
    return { entrada: conDuplicados, salida: unicos };
  })(),
  // la etiqueta de un fix no aplicado: los automáticos llevan sufijo
  etiquetas: [
    { descripcion: "Aceptar EULA (eula=true)", considerable: false },
    { descripcion: "Cambiar el puerto", considerable: true },
    { descripcion: "Subir la RAM", considerable: true },
    { descripcion: "Poner sync-chunk-writes=false", considerable: false },
  ].map(({ descripcion, considerable }) => ({
    descripcion,
    considerable,
    etiqueta: considerable ? descripcion : `${descripcion} (automático)`,
  })),
};
writeFileSync(join(golden, "doctor.json"), JSON.stringify(doctorGolden, null, 2) + "\n", "utf8");

// Engines: el mapeo de las respuestas de las APIs de descarga. La respuesta en sí
// no se puede comparar (cambia sin avisar), el mapeo sí. `salida_json` es
// `JSON.stringify` del resultado y no el valor suelto: así la comparación en Rust
// es byte a byte **incluido el orden de las claves**, que es lo que decide si un
// plan se puede reimportar.
//
// El mensaje de error solo se guarda cuando lo escribimos nosotros. Los que
// genera V8 (`raw.map is not a function`) cambian entre versiones de Node y ya
// rompieron este golden una vez: esos casos se prueban por el resultado.
const goldenEngines = (() => {
  // Cada caso guarda sus **argumentos posicionales**, no solo la entrada: el test
  // de Rust llama a su función con los mismos y compara la salida serializada.
  const captura = (nombre: string, args: unknown[], fn: () => unknown) => {
    try {
      return { nombre, args, salida_json: JSON.stringify(fn()) ?? "null", error: null as string | null };
    } catch (e) {
      return { nombre, args, salida_json: null, error: e instanceof Error ? e.message : String(e) };
    }
  };
  const caso1 = <A>(nombre: string, a1: A, fn: (a1: A) => unknown) => captura(nombre, [a1], () => fn(a1));
  const caso2 = <A, B>(nombre: string, a1: A, a2: B, fn: (a1: A, a2: B) => unknown) => captura(nombre, [a1, a2], () => fn(a1, a2));
  const caso3 = <A, B, C>(nombre: string, a1: A, a2: B, a3: C, fn: (a1: A, a2: B, a3: C) => unknown) =>
    captura(nombre, [a1, a2, a3], () => fn(a1, a2, a3));
  const caso4 = <A, B, C, D>(nombre: string, a1: A, a2: B, a3: C, a4: D, fn: (a1: A, a2: B, a3: C, a4: D) => unknown) =>
    captura(nombre, [a1, a2, a3, a4], () => fn(a1, a2, a3, a4));

  const paperV3 = (id: number, downloads: Record<string, { url: string; sha256?: string; name?: string }>) => ({ id, downloads });
  const serverDefault = { "server:default": { url: "https://fill.papermc.io/x/paper.jar", sha256: "abc123", name: "paper.jar" } };
  const v3Lista = [5, 6, 7, 6].map((id) => paperV3(id, serverDefault));
  const v3SoloMapped = paperV3(3, { "server:mojang-mapped": { url: "https://fill.papermc.io/x/mapped.jar" } });
  const v3SinSha = paperV3(3, { "server:default": { url: "https://x/y.jar" } });
  const v2Larga = {
    builds: [
      { build: 1, downloads: { application: { url: "u1", name: "n1" } } },
      { build: 2, downloads: { application: { url: "u2", name: "n2", sha256: "s2" } } },
    ],
  };
  const v2Corta = { builds: [{ build: 1, downloads: { application: { url: "u1", name: "n1" } } }] };
  const purpurInfo = { build: { build: "1234", download: "x" } };
  const forgeXml = `<metadata><versioning><versions><version>1.20.1-47.1.0</version><version>1.20.1-47.2.0</version><version>1.20.1-47.3.0</version></versions></versioning></metadata>`;
  const neoforgeLista = { versions: ["20.2.1", "20.2.2", "20.3.1", "21.1.0", "20.2.1"] };

  return {
    encode_uri_component: ["1.20.1", "paper", "a b", "ñ/é", "1.20.1+build", "a'b(c)!*~-_.", "日本", "", "100%"].map((entrada) => ({
      entrada,
      salida: encodeURIComponent(entrada),
    })),
    paper_builds_from: [
      caso1("ascendente", [{ id: 1 }, { id: 2 }, { id: 3 }], (raw) => paperBuildsFrom(raw)),
      caso1("orden raro", [{ id: 9 }, { id: 10 }, { id: 2 }], (raw) => paperBuildsFrom(raw)),
      caso1("con canal", [{ id: 1, channel: "STABLE" }, { id: 2, channel: "BETA" }], (raw) => paperBuildsFrom(raw)),
      caso1("repetidas", [{ id: 7 }, { id: 7 }], (raw) => paperBuildsFrom(raw)),
      caso1("vacía", [], (raw) => paperBuildsFrom(raw)),
      caso1("no es lista", { id: 1 }, (raw) => paperBuildsFrom(raw as never)),
      caso1("id no numérico", [{ id: "abc" }, { id: 3 }], (raw) => paperBuildsFrom(raw as never)),
    ],
    papermc_v3_from: [
      caso3("build suelta", paperV3(42, serverDefault), "paper", "1.20.1", (raw, project, mc) => papermcV3From(raw, project, mc)),
      caso3("lista de una", [paperV3(42, serverDefault)], "paper", "1.20.1", (raw, project, mc) => papermcV3From(raw, project, mc)),
      caso3("lista vacía", [], "paper", "1.20.1", (raw, project, mc) => papermcV3From(raw, project, mc)),
      // el reduce: con [5,6,7,6] "la última" es la 5, no la 7
      caso3("el reduce no es el máximo", v3Lista, "paper", "1.20.1", (raw, project, mc) => papermcV3From(raw, project, mc)),
      caso3("sin server:default", v3SoloMapped, "folia", "1.21.4", (raw, project, mc) => papermcV3From(raw, project, mc)),
      caso3("sin descargas", paperV3(3, {}), "paper", "1.20.1", (raw, project, mc) => papermcV3From(raw, project, mc)),
      caso3("sin sha256", v3SinSha, "paper", "1.20.1", (raw, project, mc) => papermcV3From(raw, project, mc)),
    ],
    papermc_v2_from: [
      caso4("sin build pedida", v2Larga, "paper", "1.20.1", null, (data, project, mc, build) => papermcV2From(data, project, mc, build ?? undefined)),
      caso4("build pedida", v2Larga, "paper", "1.20.1", "1", (data, project, mc, build) => papermcV2From(data, project, mc, build ?? undefined)),
      caso4("build que no está", v2Corta, "paper", "1.20.1", "99", (data, project, mc, build) => papermcV2From(data, project, mc, build ?? undefined)),
      caso4("lista vacía", { builds: [] }, "paper", "1.20.1", null, (data, project, mc, build) => papermcV2From(data, project, mc, build ?? undefined)),
    ],
    purpur_builds_from: [
      caso1("varias", { builds: { all: ["1", "2", "3"] } }, (data) => purpurBuildsFrom(data)),
      caso1("sin builds", {}, (data) => purpurBuildsFrom(data)),
      caso1("all vacío", { builds: { all: [] } }, (data) => purpurBuildsFrom(data)),
    ],
    purpur_download_from: [
      caso3("build por defecto", purpurInfo, "1.20.1", null, (info, mc, build) => purpurDownloadFrom(info, mc, build ?? undefined)),
      caso3("build pedida", purpurInfo, "1.20.1", "99", (info, mc, build) => purpurDownloadFrom(info, mc, build ?? undefined)),
    ],
    latest_version_from: [
      caso3("primera sin stable", [{ version: "1" }, { version: "2" }], "version", "https://meta/x", (l, f, u) => latestVersionFrom(l, f, u)),
      caso3("salta las beta", [{ version: "1", stable: false }, { version: "2" }], "version", "https://meta/x", (l, f, u) =>
        latestVersionFrom(l, f, u)
      ),
      // `String(undefined)` es "undefined": el fallo del TS se reproduce, no se arregla
      caso3("campo ausente", [{ otro: 1 }], "version", "https://meta/x", (l, f, u) => latestVersionFrom(l, f, u)),
      caso3("lista vacía", [], "version", "https://meta/x", (l, f, u) => latestVersionFrom(l, f, u)),
      caso3("casi ninguna estable", [{ version: "1", stable: false }], "version", "https://meta/x", (l, f, u) =>
        latestVersionFrom(l, f, u)
      ),
    ],
    fabric_loader_versions_from: [
      caso1("con loader", [{ loader: { version: "0.16.9" }, stable: true }], (l) => fabricLoaderVersionsFrom(l)),
      caso1("sin loader", [{ version: "0.15.0" }], (l) => fabricLoaderVersionsFrom(l)),
      caso1("beta", [{ loader: { version: "0.17.0-beta.1" }, stable: false }], (l) => fabricLoaderVersionsFrom(l)),
      caso1("loader sin version", [{ loader: {}, version: "0.14.0" }], (l) => fabricLoaderVersionsFrom(l as never)),
      caso1("vacía", [], (l) => fabricLoaderVersionsFrom(l)),
    ],
    fabric_installer_versions_from: [
      caso1("varias", [{ version: "1.0.1", stable: true }, { version: "1.1.0", stable: false }], (l) => fabricInstallerVersionsFrom(l)),
      caso1("sin version", [{ stable: true }], (l) => fabricInstallerVersionsFrom(l as never)),
    ],
    fabric_download_from: [
      caso3("normal", "1.20.1", "0.16.9", "1.0.1", (mc, loader, installer) => fabricDownloadFrom(mc, loader, installer)),
      caso3("mc con espacios", "1.20 1", "0.16.9", "1.0.1", (mc, loader, installer) => fabricDownloadFrom(mc, loader, installer)),
    ],
    quilt_loader_versions_from: [
      caso1("con loader", [{ loader: { version: "0.27.0" } }], (l) => quiltLoaderVersionsFrom(l)),
      caso1("sin loader", [{ version: "0.26.0" }], (l) => quiltLoaderVersionsFrom(l)),
    ],
    quilt_installer_versions_from: [caso1("varias", [{ version: "0.9.2" }], (l) => quiltInstallerVersionsFrom(l))],
    quilt_download_from: [caso3("normal", "1.21", "0.27.0", "0.9.2", (mc, loader, installer) => quiltDownloadFrom(mc, loader, installer))],
    forge_builds_from: [
      caso2("metadata normal", forgeXml, "1.20.1", (xml, mc) => forgeBuildsFrom(xml, mc)),
      // el prefijo va sin escapar: los puntos son comodines y esto entra igual
      caso2("los puntos son comodines", `<version>1x20y1-47.2.0</version>`, "1.20.1", (xml, mc) => forgeBuildsFrom(xml, mc)),
      // el Set quita el duplicado por la primera aparición y luego se invierte
      caso2("duplicadas", `<version>1.20.1-a</version><version>1.20.1-b</version><version>1.20.1-a</version>`, "1.20.1", (xml, mc) =>
        forgeBuildsFrom(xml, mc)
      ),
      caso2("otra mc", `<version>1.20.1-47.2.0</version>`, "1.19.2", (xml, mc) => forgeBuildsFrom(xml, mc)),
      caso2("sin versiones", `<metadata/>`, "1.20.1", (xml, mc) => forgeBuildsFrom(xml, mc)),
    ],
    forge_pick_from: [
      caso2("recommended gana", { promos: { "1.20.1-recommended": "47.2.0", "1.20.1-latest": "47.3.0" } }, "1.20.1", (p, mc) =>
        forgePickFrom(p, mc)
      ),
      caso2("solo latest", { promos: { "1.20.1-latest": "47.3.0" } }, "1.20.1", (p, mc) => forgePickFrom(p, mc)),
      caso2("ninguna", { promos: {} }, "1.20.1", (p, mc) => forgePickFrom(p, mc)),
      caso2("vacía no vale", { promos: { "1.20.1-recommended": "" } }, "1.20.1", (p, mc) => forgePickFrom(p, mc)),
    ],
    forge_download_from: [caso2("normal", "1.20.1", "47.2.0", (mc, chosen) => forgeDownloadFrom(mc, chosen))],
    neoforge_prefix: ["1.20.2", "1.21.4", "1", "20.2", ""].map((entrada) => ({ entrada, salida: neoforgePrefix(entrada) })),
    neoforge_versions_from: [
      caso2("filtra por prefijo", neoforgeLista, "1.20.2", (data, mc) => neoforgeVersionsFrom(data, mc)),
      caso2("sin candidatas", { versions: ["20.2.1"] }, "1.21.4", (data, mc) => neoforgeVersionsFrom(data, mc)),
      // el prefijo de "1" es "": `startsWith(".")` no lo cumple ninguna
      caso2("mc sin segmentos", { versions: ["20.2.1"] }, "1", (data, mc) => neoforgeVersionsFrom(data, mc)),
    ],
    neoforge_builds_from: [caso2("invierte", { versions: ["20.2.1", "20.2.2"] }, "1.20.2", (data, mc) => neoforgeBuildsFrom(data, mc))],
    neoforge_download_from: [caso2("normal", "1.20.2", "20.2.2", (mc, chosen) => neoforgeDownloadFrom(mc, chosen))],
  };
})();
mkdirSync(goldenApi, { recursive: true });
writeFileSync(join(goldenApi, "engines.json"), JSON.stringify(goldenEngines, null, 2) + "\n", "utf8");

console.log(
  `golden: ${Object.keys(sueltos).length} generadores de hoja + ${Object.keys(proyecto).length} ficheros de proyecto + catálogo + ${tiersGolden.casos.length} perfiles + ${rconGolden.casos.length} paquetes RCON + ${benchGolden.cpuScore.length} scores + ${modpackGolden.mrpack.length + modpackGolden.curseforge.length + modpackGolden.noModpack.length} casos de modpack + ${doctorGolden.scores.length} scores del doctor + ${goldenEngines.encode_uri_component.length + Object.values(goldenEngines).reduce((n, v) => n + (Array.isArray(v) ? v.length : 0), 0)} casos de engines`
);
console.log(`en ${golden}`);
