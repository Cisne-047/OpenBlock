import type { ServerConfig } from "../types";

function escPropValue(v: string): string {
  return v.replace(/\\/g, "\\\\").replace(/\n/g, "\\n");
}

/**
 * server.properties completo con los valores tuned. sync-chunk-writes=false y
 * pause-when-empty-seconds son las dos victorias gratis del vanilla moderno.
 */
export function serverProperties(cfg: ServerConfig): string {
  const aggressive = cfg.performance === "aggressive";
  const lines: Array<[string, string | number | boolean]> = [
    ["accepts-transfers", false],
    ["allow-flight", false],
    ["allow-nether", true],
    ["broadcast-console-to-ops", true],
    ["broadcast-rcon-to-ops", true],
    ["difficulty", cfg.difficulty],
    ["enable-command-block", false],
    ["enable-jmx-monitoring", false],
    ["enable-query", false],
    ["enable-rcon", cfg.rcon.enabled],
    ["enable-status", true],
    ["enforce-secure-profile", true],
    ["enforce-whitelist", cfg.whitelist],
    // rango de broadcast: 75% en aggressive reduce paquetes sin notarse
    ["entity-broadcast-range-percentage", aggressive ? 75 : 100],
    ["force-gamemode", false],
    ["function-permission-level", 2],
    ["gamemode", cfg.gamemode],
    ["generate-structures", cfg.worldType !== "flat"],
    ["hardcore", cfg.hardcore],
    ["hide-online-players", false],
    ["initial-disabled-packs", ""],
    ["initial-enabled-packs", "vanilla"],
    ["level-name", "world"],
    ["level-seed", cfg.seed],
    ["level-type", `minecraft\\:${cfg.worldType}`],
    ["log-ips", true],
    ["max-chunk-sends-per-tick", aggressive ? 16 : 24],
    ["max-chunk-receives-per-tick", aggressive ? 8 : 12],
    ["max-players", cfg.players],
    ["max-tick-time", 60000],
    ["max-world-size", 29999984],
    ["motd", escPropValue(cfg.motd)],
    ["network-compression-threshold", aggressive ? 512 : 256],
    ["online-mode", cfg.onlineMode],
    ["op-permission-level", 4],
    // pausa el tick interno cuando no queda nadie: ahorra CPU y disco
    ["pause-when-empty-seconds", aggressive ? 30 : 60],
    ["player-idle-timeout", 0],
    ["prevent-proxy-connections", false],
    ["pvp", cfg.pvp],
    ["query.port", cfg.port],
    ["rate-limit", 0],
    ["rcon.password", cfg.rcon.password],
    ["rcon.port", cfg.rcon.port],
    ["region-file-compression", "deflate"],
    ["require-resource-pack", false],
    ["resource-pack", ""],
    ["resource-pack-prompt", ""],
    ["resource-pack-sha1", ""],
    ["server-ip", ""],
    ["server-port", cfg.port],
    ["simulation-distance", cfg.simulationDistance],
    ["spawn-monsters", true],
    ["spawn-protection", 16],
    // IO síncrono de chunks: la mayor regresión de rendimiento en discos lentos
    ["sync-chunk-writes", false],
    ["text-filtering-config", ""],
    ["text-filtering-version", 0],
    ["use-native-transport", true],
    ["view-distance", cfg.viewDistance],
    ["white-list", cfg.whitelist],
  ];
  return lines.map(([k, v]) => `${k}=${v}`).join("\n") + "\n";
}
