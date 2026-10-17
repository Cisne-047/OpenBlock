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
import { planFor } from "./helpers.js";
import type { ServerConfig } from "../src/types.js";

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

console.log(`golden: ${Object.keys(sueltos).length} generadores de hoja + ${Object.keys(proyecto).length} ficheros de proyecto + catálogo`);
console.log(`en ${golden}`);
