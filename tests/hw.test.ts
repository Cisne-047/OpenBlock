import { describe, expect, it } from "vitest";
import { classifyHardware, recommendedRamMB } from "../src/hw/tiers";
import { aikarFlags, gcThreadCounts, jvmFlags, zgcFlags } from "../src/generators/jvm";
import type { HardwareProfile, TierId } from "../src/types";

const hw = (over: Partial<HardwareProfile> = {}): HardwareProfile => ({
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
  ...over,
});

describe("classifyHardware", () => {
  it("potato: poca RAM o 1 core", () => {
    expect(classifyHardware(hw({ ramTotalMB: 2048 })).id).toBe("potato");
    expect(classifyHardware(hw({ cores: 1 })).id).toBe("potato");
  });

  it("enthusiast: muchos cores + NVMe + RAM", () => {
    expect(classifyHardware(hw({ cores: 24, ramTotalMB: 65536, diskKind: "nvme" })).id).toBe("enthusiast");
  });

  it("hdd penaliza al menos un tier", () => {
    expect(classifyHardware(hw({ diskKind: "hdd", ramTotalMB: 32768, cores: 12 })).id).not.toBe("enthusiast");
  });

  it("VM penaliza", () => {
    const order: TierId[] = ["enthusiast", "high", "mid", "low", "potato"];
    const bare = order.indexOf(classifyHardware(hw({ virtualized: false })).id);
    const vm = order.indexOf(classifyHardware(hw({ virtualized: true })).id);
    expect(vm).toBeGreaterThanOrEqual(bare);
  });
});

describe("recommendedRamMB", () => {
  it("dedicada asigna más que compartida", () => {
    expect(recommendedRamMB(hw({ ramTotalMB: 32768 }), true)).toBeGreaterThan(
      recommendedRamMB(hw({ ramTotalMB: 32768 }), false)
    );
  });

  it("nunca deja el sistema sin aire", () => {
    // 4GB: 60% = 2400MB pero el cap (4096-1536) manda
    expect(recommendedRamMB(hw({ ramTotalMB: 4096 }), true)).toBeLessThanOrEqual(4096 - 1536);
    expect(recommendedRamMB(hw({ ramTotalMB: 2048 }), true)).toBeGreaterThanOrEqual(1024);
  });
});

describe("gcThreadCounts", () => {
  it("fórmula de Aikar", () => {
    expect(gcThreadCounts(4)).toEqual({ parallel: 4, conc: 3 });
    expect(gcThreadCounts(8)).toEqual({ parallel: 8, conc: 6 });
    expect(gcThreadCounts(24)).toEqual({ parallel: 18, conc: 14 });
  });
});

describe("aikarFlags", () => {
  it("heap simétrico y región según tamaño", () => {
    const small = aikarFlags(8192, 8);
    expect(small).toContain("-Xms8192M");
    expect(small).toContain("-Xmx8192M");
    expect(small).toContain("-XX:G1HeapRegionSize=8M");
    expect(small).toContain("-XX:G1NewSizePercent=30");
    expect(small).toContain("-XX:MaxTenuringThreshold=1");
    const large = aikarFlags(16384, 16);
    expect(large).toContain("-XX:G1HeapRegionSize=16M");
    expect(large).toContain("-XX:G1NewSizePercent=40");
    expect(large).toContain("-XX:MaxTenuringThreshold=15");
  });
});

describe("zgcFlags", () => {
  it("ZGC generacional con SoftMaxHeapSize", () => {
    const flags = zgcFlags(32768, 16);
    expect(flags).toContain("-XX:+UseZGC");
    expect(flags).toContain("-XX:+ZGenerational");
    expect(flags).toContain("-XX:SoftMaxHeapSize=24576M");
  });
});

describe("jvmFlags", () => {
  it("metaspace extra en modded", () => {
    const paper = { ramMB: 4096 } as never;
    const forge = { ramMB: 4096 } as never;
    expect(jvmFlags({ engine: "paper", ramMB: 4096 } as never, "g1", 8).some((f) => f.includes("Metaspace"))).toBe(false);
    expect(jvmFlags({ engine: "forge", ramMB: 4096 } as never, "g1", 8).some((f) => f.includes("Metaspace"))).toBe(true);
    void paper;
    void forge;
  });
});
