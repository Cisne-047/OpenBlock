import type { ServerConfig } from "../types.js";

/**
 * Flags de JVM. G1GC sintonizado con los flags de Aikar (fórmulas exactas de
 * https://mcflags.emc.gs) o ZGC generacional (JDK 21) para heaps grandes.
 */

export function gcThreadCounts(cores: number): { parallel: number; conc: number } {
  const parallel = cores < 8 ? cores : 8 + Math.floor(((cores - 8) * 5) / 8);
  const conc = Math.max(1, Math.round(parallel * 0.75));
  return { parallel, conc };
}

export function aikarFlags(ramMB: number, cores: number): string[] {
  const heap = `${ramMB}M`;
  const large = ramMB > 12 * 1024;
  const { parallel, conc } = gcThreadCounts(cores);
  return [
    `-Xms${heap}`,
    `-Xmx${heap}`,
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:+AlwaysPreTouch",
    `-XX:ParallelGCThreads=${parallel}`,
    `-XX:ConcGCThreads=${conc}`,
    `-XX:G1NewSizePercent=${large ? 40 : 30}`,
    `-XX:G1MaxNewSizePercent=${large ? 50 : 40}`,
    `-XX:G1HeapRegionSize=${large ? "16M" : "8M"}`,
    "-XX:G1ReservePercent=20",
    "-XX:G1HeapWastePercent=5",
    "-XX:G1MixedGCCountTarget=4",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-XX:G1MixedGCLiveThresholdPercent=90",
    "-XX:G1RSetUpdatingPauseTimePercent=5",
    "-XX:SurvivorRatio=32",
    "-XX:+PerfDisableSharedMem",
    `-XX:MaxTenuringThreshold=${large ? 15 : 1}`,
    "-Dusing.aikars.flags=https://mcflags.emc.gs",
    "-Daikars.new.flags=true",
  ];
}

export function zgcFlags(ramMB: number, cores: number): string[] {
  const heap = `${ramMB}M`;
  const soft = `${Math.round((ramMB * 0.75) / 512) * 512}M`;
  const { parallel } = gcThreadCounts(cores);
  return [
    `-Xms${heap}`,
    `-Xmx${heap}`,
    "-XX:+UseZGC",
    "-XX:+ZGenerational",
    "-XX:+DisableExplicitGC",
    "-XX:+AlwaysPreTouch",
    `-XX:SoftMaxHeapSize=${soft}`,
    `-XX:ParallelGCThreads=${parallel}`,
    "-Dusing.aikars.flags=https://mcflags.emc.gs",
  ];
}

/** Flags completos del server según config (GC, heap, metaspace para modded) */
export function jvmFlags(cfg: ServerConfig, gc: "g1" | "zgc", cores: number): string[] {
  const n = Math.max(1, cores);
  const base = gc === "zgc" ? zgcFlags(cfg.ramMB, n) : aikarFlags(cfg.ramMB, n);
  const modded = ["fabric", "quilt", "forge", "neoforge"].includes(cfg.engine);
  if (!modded) return base;
  // modpacks grandes cargan muchísimo bytecode
  return [...base, "-XX:MaxMetaspaceSize=1G"];
}

/** una línea para jvm-args.txt y los scripts de arranque */
export function jvmArgsLine(cfg: ServerConfig, gc: "g1" | "zgc", cores: number): string {
  return jvmFlags(cfg, gc, cores).join(" ");
}
