import { describe, expect, it } from "vitest";
import { generateProject, validate } from "../src/generators/index.js";
import { zipProject } from "../src/output.js";
import { serverProperties } from "../src/generators/properties.js";
import { bukkitYml, paperWorldDefaultsYml, spigotYml } from "../src/generators/paper.js";
import { planFor, serverCfg as cfg } from "./helpers.js";

describe("serverProperties", () => {
  it("tuning base presente", () => {
    const p = serverProperties(cfg());
    expect(p).toContain("sync-chunk-writes=false");
    expect(p).toContain("view-distance=8");
    expect(p).toContain("pause-when-empty-seconds=60");
    expect(p).toContain("enable-rcon=true");
    expect(p).toContain("rcon.password=secreto1234");
    expect(p).toContain("online-mode=true");
  });

  it("escapa backslashes y saltos del MOTD", () => {
    const p = serverProperties(cfg({ motd: "con \\ barra\ny salto" }));
    expect(p).toContain("motd=con \\\\ barra\\ny salto");
  });

  it("aggressive ajusta compression y broadcast", () => {
    const p = serverProperties(cfg({ performance: "aggressive" }));
    expect(p).toContain("network-compression-threshold=512");
    expect(p).toContain("entity-broadcast-range-percentage=75");
  });
});

describe("paper configs", () => {
  it("ALTERNATE_CURRENT en world defaults", () => {
    expect(paperWorldDefaultsYml(cfg())).toContain("redstone-implementation: ALTERNATE_CURRENT");
  });
  it("aggressive baja entity-activation-range", () => {
    expect(spigotYml(cfg({ performance: "aggressive" }))).toContain("animals: 16");
    expect(spigotYml(cfg())).toContain("animals: 32");
  });
  it("bukkit con límites de spawn", () => {
    expect(bukkitYml(cfg())).toContain("monsters: 70");
    expect(bukkitYml(cfg({ performance: "aggressive" }))).toContain("monsters: 50");
  });
});

describe("generateProject", () => {
  it("paper: set completo de ficheros", () => {
    const files = generateProject(cfg(), planFor(), 8);
    expect(Object.keys(files)).toEqual(
      expect.arrayContaining([
        "openblock.json",
        "eula.txt",
        "server.properties",
        "start.sh",
        "start.bat",
        "jvm-args.txt",
        "paper-global.yml",
        "paper-world-defaults.yml",
        "spigot.yml",
        "bukkit.yml",
        "install.sh",
        "install.ps1",
        "firewall.sh",
        "firewall.ps1",
        "backup.sh",
        "backup.ps1",
        "openblock.service",
        "install-service.sh",
        "winsw.xml",
        "openblock-backup.timer",
        "README.md",
      ])
    );
    expect(files["start.sh"]).toContain('paper-1.21.4-100.jar');
    expect(files["start.sh"]).toContain('-jar "$JAR" --nogui');
    expect(files["start.bat"]).toContain('--nogui');
    expect(files["eula.txt"]).toBe("eula=true\n");
    // round-trip: openblock.json reparsea a config equivalente
    expect(JSON.parse(files["openblock.json"]).mcVersion).toBe("1.21.4");
  });

  it("vanilla: sin paper configs, nogui sin guiones", () => {
    const vanilla = cfg({ engine: "vanilla" });
    const files = generateProject(vanilla, planFor(vanilla), 8);
    expect(files["start.sh"]).toContain('server.jar');
    expect(files["start.sh"]).toContain(' nogui');
    expect(files["paper-global.yml"]).toBeUndefined();
  });

  it("forge: installer genera user_jvm_args.txt y run.sh en start script", () => {
    const files = generateProject(cfg({ engine: "forge" }), planFor(cfg({ engine: "forge" })), 8);
    expect(files["user_jvm_args.txt"]).toContain("-Xmx8192M");
    expect(files["start.sh"]).toContain("exec bash run.sh nogui");
    expect(files["install.sh"]).toContain("--installServer");
  });

  it("targetOS windows: scripts bash de ops fuera, start doble siempre", () => {
    const files = generateProject(cfg({ targetOS: "windows" }), planFor(), 8);
    expect(files["start.bat"]).toBeDefined();
    expect(files["start.sh"]).toBeDefined(); // start doble: permite copiar el zip a Linux
    expect(files["install.sh"]).toBeUndefined();
    expect(files["firewall.sh"]).toBeUndefined();
    expect(files["backup.sh"]).toBeUndefined();
    expect(files["openblock.service"]).toBeUndefined();
    expect(files["install.ps1"]).toBeDefined();
    expect(files["winsw.xml"]).toBeDefined();
  });
});

describe("validate", () => {
  it("coge errores obvios", () => {
    expect(validate(cfg({ port: 80 }))).toHaveLength(1);
    expect(validate(cfg({ rcon: { enabled: true, password: "corta", port: 25575 } }))).toHaveLength(1);
    expect(validate(cfg())).toHaveLength(0);
  });
});

describe("zipProject", () => {
  it("empaqueta el FileMap", async () => {
    const files = generateProject(cfg(), planFor(), 8);
    const zip = await zipProject(files);
    expect(zip.length).toBeGreaterThan(1000);
    const JSZip = (await import("jszip")).default;
    const parsed = await JSZip.loadAsync(zip);
    expect(await parsed.file("eula.txt")!.async("string")).toBe("eula=true\n");
  });
});
