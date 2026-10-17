import { describe, expect, it } from "vitest";
import { mkdtemp, readFile, writeFile, rm, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { findFreePort, jarNameFromStartScript, runDoctor } from "../src/doctor";
import type { ServerConfig } from "../src/types";

const cfg = (over: Partial<ServerConfig> = {}): ServerConfig => ({
  name: "test",
  mcVersion: "1.21.4",
  engine: "paper",
  serverDir: "/tmp/server",
  ramMB: 8192,
  players: 20,
  viewDistance: 8,
  simulationDistance: 6,
  port: 25565,
  motd: "test",
  difficulty: "normal",
  gamemode: "survival",
  worldType: "normal",
  seed: "",
  onlineMode: true,
  whitelist: false,
  pvp: true,
  hardcore: false,
  modpack: { kind: "none", url: "" },
  performance: "balanced",
  pregenRadius: 0,
  backup: { enabled: false, intervalHours: 24, keep: 7 },
  service: { enabled: false },
  rcon: { enabled: true, password: "secreto1234", port: 25575 },
  chunkyborder: false,
  targetOS: "both",
  customItems: [],
  ...over,
});

// acceso a las reglas para tests unitarios sin arrancar servers
const ctxFor = (over: Partial<ServerConfig> = {}) => {
  const c = cfg(over);
  return { cfg: c, dir: c.serverDir, logLines: [] as string[], bootTested: false, bootOk: false };
};

describe("reglas de log", () => {
  it("detecta Java incorrecto y ofrece fix automático", async () => {
    const { LOG_RULES } = await import("../src/doctor");
    const ctx = ctxFor();
    const rule = LOG_RULES.find((r) => r.code === "JAVA_VERSION")!;
    const m = "Minecraft 26.1 and newer requires running the server with Java 25 or above.".match(rule.pattern)!;
    const fix = rule.fix!(m, ctx)!;
    expect(fix.code).toBe("install-java");
    expect(fix.considerable).toBe(false);
  });

  it("detecta puerto ocupado: fix considerable (pregunta)", async () => {
    const { LOG_RULES } = await import("../src/doctor");
    const rule = LOG_RULES.find((r) => r.code === "PORT_IN_USE")!;
    const m = "Failed to bind to port!".match(rule.pattern)!;
    const fix = rule.fix!(m, ctxFor())!;
    expect(fix.considerable).toBe(true);
  });

  it("detecta OOM: subida de RAM considerable", async () => {
    const { LOG_RULES } = await import("../src/doctor");
    const rule = LOG_RULES.find((r) => r.code === "OOM")!;
    const m = "java.lang.OutOfMemoryError: Java heap space".match(rule.pattern)!;
    const fix = rule.fix!(m, ctxFor())!;
    expect(fix.code).toBe("raise-ram");
    expect(fix.considerable).toBe(true);
  });

  it("Can't keep up es warning con fix de view-distance", async () => {
    const { LOG_RULES } = await import("../src/doctor");
    const rule = LOG_RULES.find((r) => r.code === "TICK_BEHIND")!;
    expect(rule.severity).toBe("warning");
    const m = "Can't keep up! Is the server overloaded? Running 3000ms or 60 ticks behind".match(rule.pattern)!;
    const fix = rule.fix!(m, ctxFor({ viewDistance: 8 }))!;
    expect(fix.considerable).toBe(true);
  });

  it("plugin roto: fix considerable que mueve a plugins_disabled", async () => {
    const { LOG_RULES } = await import("../src/doctor");
    const rule = LOG_RULES.find((r) => r.code === "PLUGIN_CRASH")!;
    const m = "Error occurred while enabling Chunky v1.5.3".match(rule.pattern)!;
    expect(rule.fix!(m, ctxFor())!.considerable).toBe(true);
  });
});

describe("reglas estáticas + fixes reales", () => {
  it("eula ausente: fix auto escribe eula=true", async () => {
    const dir = await mkdtemp(join(tmpdir(), "ob-doctor-"));
    try {
      const { STATIC_RULES } = await import("../src/doctor");
      const ctx = { cfg: cfg({ serverDir: dir }), dir, logLines: [], bootTested: false, bootOk: false };
      const rule = STATIC_RULES.find((r) => r.code === "EULA_FILE")!;
      const f = await rule.check(ctx);
      expect(f?.severity).toBe("error");
      await f!.fix!.apply();
      expect(await readFile(join(dir, "eula.txt"), "utf8")).toBe("eula=true\n");
      // segunda pasada: ya no encuentra nada
      expect(await rule.check(ctx)).toBeNull();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("sync-chunk-writes=true: fix auto lo apaga", async () => {
    const dir = await mkdtemp(join(tmpdir(), "ob-doctor-"));
    try {
      await writeFile(join(dir, "server.properties"), "sync-chunk-writes=true\nview-distance=8\n");
      const { STATIC_RULES } = await import("../src/doctor");
      const ctx = { cfg: cfg({ serverDir: dir }), dir, logLines: [], bootTested: false, bootOk: false };
      const rule = STATIC_RULES.find((r) => r.code === "SYNC_CHUNK_WRITES")!;
      const f = await rule.check(ctx);
      expect(f?.severity).toBe("warning");
      await f!.fix!.apply();
      expect(await readFile(join(dir, "server.properties"), "utf8")).toContain("sync-chunk-writes=false");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("rcon con contraseña débil: regenera", async () => {
    const dir = await mkdtemp(join(tmpdir(), "ob-doctor-"));
    try {
      await writeFile(join(dir, "server.properties"), "enable-rcon=true\nrcon.password=\nrcon.port=25575\n");
      const { STATIC_RULES } = await import("../src/doctor");
      const ctx = { cfg: cfg({ serverDir: dir }), dir, logLines: [], bootTested: false, bootOk: false };
      const rule = STATIC_RULES.find((r) => r.code === "RCON_PASSWORD")!;
      const f = await rule.check(ctx);
      expect(f?.severity).toBe("error");
      await f!.fix!.apply();
      const txt = await readFile(join(dir, "server.properties"), "utf8");
      expect(/rcon.password=(.{8,})/m.test(txt)).toBe(true);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("jar del engine ausente: error sin fix", async () => {
    const dir = await mkdtemp(join(tmpdir(), "ob-doctor-"));
    try {
      await mkdir(join(dir, "plugins"), { recursive: true });
      const { STATIC_RULES } = await import("../src/doctor");
      const ctx = { cfg: cfg({ serverDir: dir }), dir, logLines: [], bootTested: false, bootOk: false };
      const rule = STATIC_RULES.find((r) => r.code === "ENGINE_JAR_MISSING")!;
      const f = await rule.check(ctx);
      expect(f?.severity).toBe("error");
      expect(f?.fix).toBeUndefined();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

describe("helpers", () => {
  it("jarNameFromStartScript extrae el jar del start.sh generado", async () => {
    const dir = await mkdtemp(join(tmpdir(), "ob-doctor-"));
    try {
      await writeFile(join(dir, "start.sh"), 'JAR="paper-26.3-175.jar"\nexec "$JAVA" -jar "$JAR" --nogui\n');
      expect(jarNameFromStartScript(dir)).toBe("paper-26.3-175.jar");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("findFreePort devuelve puertos realmente libres", async () => {
    const p = await findFreePort(45100);
    expect(p).toBeGreaterThanOrEqual(45100);
  });
});

describe("runDoctor end-to-end (sin boot)", () => {
  it("score y grade coherentes con findings", async () => {
    const dir = await mkdtemp(join(tmpdir(), "ob-doctor-"));
    try {
      // server vacío: eula falta (error) + jar falta (error) => score 60.
      // mcVersion 26.3 = Java 25 = el del sistema: evita que JAVA_MAJOR baje un JDK de verdad
      const cfgx = cfg({ serverDir: dir, mcVersion: "26.3" });
      const r = await runDoctor(cfgx, { boot: false });
      const codes = r.findings.map((f) => f.code);
      // lo auto-arreglado (eula, java) sale de findings y queda en applied
      expect(codes).toContain("ENGINE_JAR_MISSING");
      expect(r.findings.find((f) => f.code === "EULA_FILE")).toBeUndefined();
      expect(r.applied).toContain("accept-eula");
      expect(await readFile(join(dir, "eula.txt"), "utf8")).toBe("eula=true\n");
      // score post-fix: solo el error sin fix automático (jar ausente) resta
      expect(r.score).toBe(80);
      expect(r.grade).toBe("OK CON AVISOS");
      // eula se arregló sola (fix auto)
      expect(await readFile(join(dir, "eula.txt"), "utf8")).toBe("eula=true\n");
      expect(r.applied).toContain("accept-eula");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
