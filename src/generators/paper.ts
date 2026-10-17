import type { ServerConfig } from "../types.js";

/**
 * Configs de la familia Paper (paper/purpur/folia): paper-global.yml,
 * paper-world-defaults.yml, spigot.yml y bukkit.yml. Solo escribimos claves
 * con efecto de rendimiento conocido y estable entre 1.19-1.21: Paper migra el
 * resto a defaults con su propio versionado.
 *
 * Perfiles: conservative = toca casi nada; balanced = lo seguro; aggressive =
 * reduce rangos de activación y spawns (gameplay ligeramente distinto).
 */

const aggressive = (cfg: ServerConfig) => cfg.performance === "aggressive";

export function paperGlobalYml(_cfg: ServerConfig): string {
  return `chunk-loading-advanced:
  auto-config-send-distance: false
  player-max-concurrent-chunk-generates: 0
  player-max-concurrent-chunk-loads: 0
chunk-system:
  gen-parallelism: default
  io-threads: -1
  worker-threads: -1
misc:
  region-file-cache-size: 256
watchdog:
  early-warning-delay: 10000
  early-warning-every: 5000
`;
}

export function paperWorldDefaultsYml(cfg: ServerConfig): string {
  const aggr = aggressive(cfg);
  return `chunks:
  auto-save-interval: 6000
  delay-chunk-unload-by: 10s
  fixed-chunk-inhabited-time: -1
  max-auto-save-chunks-per-tick: 24
  prevent-moving-into-unloaded-chunks: false
entities:
  behavior:
    optimize-explosions: true
  spawning:
    alt-item-despawn-rate:
      enabled: true
      items:
        COBBLESTONE: 300
    per-player-mob-spawns: true
hopper:
  cooldown-when-full: true
  disable-move-event: false
misc:
  redstone-implementation: ALTERNATE_CURRENT
  random-light-updates: false
tick-rates:
  container-update: 1
  mob-spawner: ${aggr ? 2 : 1}
`;
}

export function spigotYml(cfg: ServerConfig): string {
  const aggr = aggressive(cfg);
  const netty = cfg.players > 20 ? 4 : 2;
  return `settings:
  log-villager-deaths: true
  netty-threads: ${netty}
  timeout-time: 60
  restart-on-crash: true
  restart-script: ./start.sh
world-settings:
  default:
    arrow-despawn-rate: 300
    entity-activation-range:
      animals: ${aggr ? 16 : 32}
      monsters: ${aggr ? 24 : 32}
      raiders: ${aggr ? 32 : 48}
      misc: ${aggr ? 8 : 16}
      tick-inactive-villagers: true
    item-despawn-rate: 6000
    merge-radius:
      item: 2.5
      exp: 3.0
    max-tnt-per-tick: ${aggr ? 50 : 100}
    verbose: false
`;
}

export function bukkitYml(cfg: ServerConfig): string {
  const aggr = aggressive(cfg);
  return `settings:
  warn-on-overload: true
spawn-limits:
  monsters: ${aggr ? 50 : 70}
  animals: 10
  water-animals: 5
  water-ambient: 20
  ambient: 15
ticks-per:
  monster-spawns: ${aggr ? 2 : 1}
  animal-spawns: 400
  autosave: 6000
chunk-gc:
  period-in-ticks: 600
`;
}
