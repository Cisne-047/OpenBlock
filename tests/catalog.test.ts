import { describe, expect, it } from "vitest";
import { engineById, javaMajorFor, parseMc, perfItemsFor, versionKey } from "../src/catalog";

describe("parseMc", () => {
  it("parsea versiones con y sin patch", () => {
    expect(parseMc("1.21.4")).toEqual([1, 21, 4]);
    expect(parseMc("1.20")).toEqual([1, 20, 0]);
    expect(parseMc("26w14a")).toEqual([0, 0, 0]);
  });
});

describe("javaMajorFor", () => {
  it("mapa de versiones de Java por MC", () => {
    expect(javaMajorFor("26.3")).toBe(25);
    expect(javaMajorFor("26.1")).toBe(25);
    expect(javaMajorFor("1.21.4")).toBe(21);
    expect(javaMajorFor("1.20.6")).toBe(21);
    expect(javaMajorFor("1.20.5")).toBe(21);
    expect(javaMajorFor("1.20.4")).toBe(17);
    expect(javaMajorFor("1.18.2")).toBe(17);
    expect(javaMajorFor("1.17.1")).toBe(17);
    expect(javaMajorFor("1.16.5")).toBe(8);
    expect(javaMajorFor("1.12.2")).toBe(8);
  });
});

describe("versionKey", () => {
  it("ordena correctamente", () => {
    expect(versionKey([1, 21, 4])).toBeGreaterThan(versionKey([1, 20, 5]));
    expect(versionKey([1, 20, 5])).toBeGreaterThan(versionKey([1, 20, 4]));
  });
});

describe("perfItemsFor", () => {
  it("paper recibe plugins, no mods de fabric", () => {
    const items = perfItemsFor("paper", "1.21.4");
    const ids = items.map((i) => i.id);
    expect(ids).toContain("spark");
    expect(ids).toContain("chunky");
    expect(ids).not.toContain("lithium");
    expect(ids).not.toContain("c2me");
  });

  it("fabric 1.21 recibe lithium, c2me, krypton", () => {
    const ids = perfItemsFor("fabric", "1.21.4").map((i) => i.id);
    expect(ids).toContain("lithium");
    expect(ids).toContain("c2me");
    expect(ids).toContain("krypton");
    expect(ids).not.toContain("canary");
  });

  it("lazydfu solo en <=1.19.4", () => {
    expect(perfItemsFor("fabric", "1.19.4").map((i) => i.id)).toContain("lazydfu");
    expect(perfItemsFor("fabric", "1.20.4").map((i) => i.id)).not.toContain("lazydfu");
  });

  it("canary (forge) acotado a 1.18.2-1.20.1", () => {
    expect(perfItemsFor("forge", "1.20.1").map((i) => i.id)).toContain("canary");
    expect(perfItemsFor("forge", "1.21.4").map((i) => i.id)).not.toContain("canary");
    expect(perfItemsFor("forge", "1.18.2").map((i) => i.id)).toContain("canary");
    expect(perfItemsFor("forge", "1.16.5").map((i) => i.id)).not.toContain("canary");
  });

  it("vanilla no recibe nada", () => {
    expect(perfItemsFor("none", "1.21.4")).toHaveLength(0);
  });
});

describe("engineById", () => {
  it("folia es multithread, paper no", () => {
    expect(engineById("folia").multithread).toBe(true);
    expect(engineById("paper").multithread).toBe(false);
    expect(engineById("forge").installer).toBe(true);
  });
});
