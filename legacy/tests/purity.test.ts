import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { generateProject } from "../src/generators/index.js";
import { planFor, serverCfg } from "./helpers.js";

/**
 * Estos dos invariantes no son míos: son los de DistroForge, y son los que
 * hacen que el port a Rust sea verificable en vez de una promesa.
 *
 * 1. `forge-core` no contiene una sola referencia a `std::fs`, `std::io`,
 *    `std::process` ni `std::env`: la E/S vive solo en los bordes. Aquí el
 *    equivalente es que `src/generators/` no importe `node:*`.
 * 2. Su suite recorre todas las bases × todos los ficheros generados buscando
 *    la subcadena "undefined" (`tests/generators.rs:73-79`), porque un
 *    `undefined` que se cuela en una receta la rompe en silencio. Aquí es
 *    peor todavía: los ficheros generados son scripts de shell y YAML que se
 *    ejecutan en la máquina del usuario.
 */

const GENERATORS_DIR = join(__dirname, "..", "src", "generators");

describe("pureza del núcleo", () => {
  it("ningún fichero de generators/ importa node:* (la E/S vive en output.ts)", () => {
    const ficheros = readdirSync(GENERATORS_DIR).filter((f) => f.endsWith(".ts"));
    expect(ficheros.length).toBeGreaterThan(4);

    const conNode = ficheros.filter((f) => /from\s+"node:/.test(readFileSync(join(GENERATORS_DIR, f), "utf8")));
    expect(conNode, `generators/ debe ser puro; importan node:*: ${conNode.join(", ")}`).toEqual([]);
  });

  it("los generadores son deterministas: misma entrada, mismo FileMap", () => {
    const a = generateProject(serverCfg(), planFor(), 8);
    const b = generateProject(serverCfg(), planFor(), 8);
    expect(a).toEqual(b);
  });
});

describe("barrido de undefined", () => {
  // matriz de configs: los caminos que activan ramas distintas del generador
  const casos: Array<[string, Parameters<typeof generateProject>[0]]> = [
    ["paper por defecto", serverCfg()],
    ["vanilla (sin pluginApi)", serverCfg({ engine: "vanilla" })],
    ["forge (con installer)", serverCfg({ engine: "forge" })],
    ["solo linux", serverCfg({ targetOS: "linux" })],
    ["solo windows", serverCfg({ targetOS: "windows" })],
    ["sin backup ni servicio", serverCfg({ backup: { enabled: false, intervalHours: 24, keep: 7 }, service: { enabled: false } })],
    ["rcon apagado", serverCfg({ rcon: { enabled: false, password: "", port: 25575 } })],
    ["con plugins propios", serverCfg({ customItems: [{ name: "mi-plugin", kind: "plugin", url: "https://ejemplo/mi.jar" }] })],
    ["con ruta java explícita", serverCfg({ javaPath: "/opt/java/bin/java" })],
    ["conservador", serverCfg({ performance: "conservative" })],
    ["agresivo", serverCfg({ performance: "aggressive" })],
  ];

  for (const [nombre, cfg] of casos) {
    it(`${nombre}: ningún fichero contiene "undefined"`, () => {
      const files = generateProject(cfg, planFor(cfg), cfg.engine === "forge" ? 8 : 8);
      const fugas = Object.entries(files)
        .filter(([, contenido]) => contenido.includes("undefined"))
        .map(([ruta]) => ruta);
      expect(fugas, `"undefined" se coló en: ${fugas.join(", ")}`).toEqual([]);
    });
  }

  it("ningún fichero generado sale vacío", () => {
    const files = generateProject(serverCfg(), planFor(), 8);
    const vacios = Object.entries(files)
      .filter(([, contenido]) => contenido.trim().length < 3)
      .map(([ruta]) => ruta);
    expect(vacios, `ficheros vacíos: ${vacios.join(", ")}`).toEqual([]);
  });
});
