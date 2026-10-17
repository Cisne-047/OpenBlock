import { describe, expect, it } from "vitest";
import { decodePacket, encodePacket } from "../src/rcon";
import { cfLoaderKind, mrpackServerFiles, parseCfManifest, parseMrpackIndex } from "../src/api/modrinth";

describe("rcon codec", () => {
  it("roundtrip auth", () => {
    const buf = encodePacket(7, 3, "pass123");
    expect(buf.readInt32LE(0)).toBe(4 + 4 + 7 + 2);
    const pkt = decodePacket(buf);
    expect(pkt).toEqual({ id: 7, type: 3, payload: "pass123" });
  });

  it("roundtrip comando con espacios", () => {
    const buf = encodePacket(1, 2, "chunky radius 4000");
    expect(decodePacket(buf).payload).toBe("chunky radius 4000");
  });
});

const MRPACK_INDEX = JSON.stringify({
  formatVersion: 1,
  game: "minecraft",
  versionId: "1.0.0",
  name: "Pack de prueba",
  files: [
    {
      path: "mods/lithium.jar",
      env: { client: "required", server: "required" },
      downloads: ["https://cdn.modrinth.com/lithium.jar"],
      hashes: { sha1: "aa", sha512: "bb" },
      fileSize: 100,
    },
    {
      path: "mods/sodium-client.jar",
      env: { client: "required", server: "unsupported" },
      downloads: ["https://cdn.modrinth.com/sodium.jar"],
      hashes: {},
      fileSize: 100,
    },
    {
      path: "config/optimizado.toml",
      downloads: ["https://cdn.modrinth.com/cfg.toml"],
      hashes: {},
      fileSize: 10,
    },
  ],
  dependencies: { minecraft: "1.21.1", "fabric-loader": "0.16.9" },
});

describe("mrpack", () => {
  it("parsea índice y filtra client-only", () => {
    const idx = parseMrpackIndex(MRPACK_INDEX);
    const server = mrpackServerFiles(idx);
    expect(server.map((f) => f.path)).toEqual(["mods/lithium.jar", "config/optimizado.toml"]);
    expect(idx.dependencies.minecraft).toBe("1.21.1");
  });
});

const CF_MANIFEST = JSON.stringify({
  manifestType: "minecraftModpack",
  manifestVersion: 1,
  name: "Pack CF",
  minecraft: { version: "1.20.1", modLoaders: [{ id: "forge-47.2.0", primary: true }] },
  files: [{ projectID: 1, fileID: 2, required: true }],
  overrides: "overrides",
});

describe("curseforge manifest", () => {
  it("detecta loader y build", () => {
    const m = parseCfManifest(CF_MANIFEST);
    expect(cfLoaderKind(m)).toEqual({ engine: "forge", build: "47.2.0" });
  });
  it("rechaza manifests no-modpack", () => {
    expect(() => parseCfManifest(JSON.stringify({ manifestType: "otro" }))).toThrow();
  });
});
