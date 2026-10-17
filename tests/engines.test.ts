import { describe, expect, it } from "vitest";
import {
  fabricDownloadFrom,
  fabricInstallerVersionsFrom,
  forgeBuildsFrom,
  forgePickFrom,
  latestVersionFrom,
  neoforgeVersionsFrom,
  paperBuildsFrom,
  papermcV2From,
  papermcV3From,
  purpurBuildsFrom,
  purpurDownloadFrom,
} from "../src/api/engines.js";

/**
 * Estas pruebas no describen lo que *debería* hacer un resolvedor de descargas:
 * describen lo que hace el que hay. Las rarezas están reproducidas a propósito
 * en Rust (`rust/crates/forge-api/src/engines.rs`), y el golden de paridad las
 * compara. Si alguien "arregla" una de aquí, el puerto deja de ser un puerto y
 * esta prueba es el sitio donde se ve.
 */
describe("engines: las rarezas que el port reproduce", () => {
  it("el reduce de paper v3 no es el máximo: el acumulador vuelve a builds[0]", () => {
    const builds = [5, 6, 7, 6].map((id) => ({ id, downloads: { "server:default": { url: "u" } } }));
    // con [5,6,7,6] la 6 última no supera a la 7, así que cae a builds[0]: la 5
    expect(papermcV3From(builds, "paper", "1.20.1")?.resolved).toBe("build 5");
    // y en orden ascendente sí sale la última
    const asc = [5, 6, 7].map((id) => ({ id, downloads: { "server:default": { url: "u" } } }));
    expect(papermcV3From(asc, "paper", "1.20.1")?.resolved).toBe("build 7");
  });

  it("las descargas se eligen por orden de escritura, no por orden alfabético", () => {
    const salida = papermcV3From({ id: 1, downloads: { "zeta:default": { url: "z" }, "alpha:default": { url: "a" } } }, "paper", "1.21");
    expect(salida?.url).toBe("z");
    // y `server:default` gana a cualquiera
    const conServer = papermcV3From({ id: 1, downloads: { "zeta:default": { url: "z" }, "server:default": { url: "s" } } }, "paper", "1.21");
    expect(conServer?.url).toBe("s");
  });

  it("una entrada sin `stable` cuenta como estable", () => {
    // no es "la primera estable", es "la primera que no diga stable: false"
    expect(latestVersionFrom([{ version: "1" }, { version: "2" }], "version", "u")).toBe("1");
    expect(latestVersionFrom([{ version: "1", stable: false }, { version: "2" }], "version", "u")).toBe("2");
    // y si todas son beta, se cuela la primera de todas
    expect(latestVersionFrom([{ version: "1", stable: false }], "version", "u")).toBe("1");
  });

  it("un campo que no está acaba escrito como \"undefined\"", () => {
    expect(latestVersionFrom([{ otro: 1 }], "version", "u")).toBe("undefined");
  });

  it("sin lista de builds, el mensaje dice undefined", () => {
    expect(() => papermcV2From({ builds: [] }, "paper", "1.20.1")).toThrow("Sin build undefined de paper para 1.20.1");
    expect(() => papermcV2From({ builds: [] }, "paper", "1.20.1", "9")).toThrow("Sin build 9 de paper para 1.20.1");
  });

  it("las builds de paper se ordenan por número, no por texto", () => {
    expect(paperBuildsFrom([{ id: 9 }, { id: 10 }, { id: 2 }]).map((b) => b.id)).toEqual(["10", "9", "2"]);
  });

  it("purpur devuelve las builds al revés de como vienen", () => {
    expect(purpurBuildsFrom({ builds: { all: ["1", "2", "3"] } }).map((b) => b.id)).toEqual(["3", "2", "1"]);
    expect(purpurBuildsFrom({}).map((b) => b.id)).toEqual([]);
  });

  it("el prefijo del regex de forge va sin escapar", () => {
    // "1.20.1" son comodines: esto entra, y un escape lo dejaría fuera
    expect(forgeBuildsFrom("<version>1x20y1-47.2.0</version>", "1.20.1").map((b) => b.id)).toEqual(["47.2.0"]);
    expect(forgeBuildsFrom("<version>1.19.2-47.2.0</version>", "1.20.1")).toEqual([]);
  });

  it("forge deduplica por la primera aparición y después invierte", () => {
    const xml = "<version>1.20.1-a</version><version>1.20.1-b</version><version>1.20.1-a</version>";
    expect(forgeBuildsFrom(xml, "1.20.1").map((b) => b.id)).toEqual(["b", "a"]);
  });

  it("forge prefiere recommended, y una cadena vacía no vale", () => {
    expect(forgePickFrom({ promos: { "1.20.1-recommended": "47.2.0", "1.20.1-latest": "47.3.0" } }, "1.20.1")).toBe("47.2.0");
    expect(forgePickFrom({ promos: { "1.20.1-latest": "47.3.0" } }, "1.20.1")).toBe("47.3.0");
    expect(forgePickFrom({ promos: {} }, "1.20.1")).toBeNull();
    expect(forgePickFrom({ promos: { "1.20.1-recommended": "" } }, "1.20.1")).toBe("");
  });

  it("neoforge pierde el primer segmento de la MC", () => {
    expect(neoforgeVersionsFrom({ versions: ["20.2.1", "21.1.0"] }, "1.20.2")).toEqual(["20.2.1"]);
    // el prefijo de "1" es "": nada empieza por "."
    expect(neoforgeVersionsFrom({ versions: ["20.2.1"] }, "1")).toEqual([]);
  });

  it("los instaladores de fabric no pasan por String(): un campo ausente desaparece", () => {
    expect(fabricInstallerVersionsFrom([{ stable: true }] as never)).toEqual([{ channel: "stable" }]);
  });

  it("la mc va escapada en la url y el loader no", () => {
    expect(fabricDownloadFrom("1.20 1", "0.16.9", "1.0.1").url).toBe(
      "https://meta.fabricmc.net/v2/versions/loader/1.20%201/0.16.9/1.0.1/server/jar"
    );
  });

  it("purpur construye la url con la build elegida", () => {
    const info = { build: { build: "1234", download: "x" } };
    expect(purpurDownloadFrom(info, "1.20.1").url).toBe("https://api.purpurmc.org/v2/purpur/1.20.1/1234/download");
    expect(purpurDownloadFrom(info, "1.20.1", "99").resolved).toBe("build 99");
  });
});
