//! Configs de la familia Paper (paper/purpur/folia), portadas de
//! `src/generators/paper.ts`.
//!
//! Solo se escriben claves con efecto de rendimiento conocido y estable entre
//! 1.19-1.21: Paper migra el resto a defaults con su propio versionado.
//!
//! Perfiles: conservative = toca casi nada; balanced = lo seguro; aggressive =
//! reduce rangos de activación y spawns (gameplay ligeramente distinto).

use crate::types::{PerfProfile, ServerConfig};

fn aggressive(cfg: &ServerConfig) -> bool {
    cfg.performance == PerfProfile::Aggressive
}

/// `paper-global.yml` (`paperGlobalYml`).
///
/// El TS recibe la config y no la usa; aquí directamente no hay parámetro, que
/// es lo que el linter del TS estaba pidiendo a gritos con ese `_cfg`.
pub fn paper_global_yml() -> String {
    r#"chunk-loading-advanced:
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
"#
    .to_string()
}

/// `paper-world-defaults.yml` (`paperWorldDefaultsYml`).
pub fn paper_world_defaults_yml(cfg: &ServerConfig) -> String {
    let mob_spawner = if aggressive(cfg) { 2 } else { 1 };
    format!(
        r#"chunks:
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
  mob-spawner: {mob_spawner}
"#
    )
}

/// `spigot.yml` (`spigotYml`).
pub fn spigot_yml(cfg: &ServerConfig) -> String {
    let aggr = aggressive(cfg);
    let netty = if cfg.players > 20 { 4 } else { 2 };
    let (animals, monsters, raiders, misc) = if aggr {
        (16, 24, 32, 8)
    } else {
        (32, 32, 48, 16)
    };
    let max_tnt = if aggr { 50 } else { 100 };
    format!(
        r#"settings:
  log-villager-deaths: true
  netty-threads: {netty}
  timeout-time: 60
  restart-on-crash: true
  restart-script: ./start.sh
world-settings:
  default:
    arrow-despawn-rate: 300
    entity-activation-range:
      animals: {animals}
      monsters: {monsters}
      raiders: {raiders}
      misc: {misc}
      tick-inactive-villagers: true
    item-despawn-rate: 6000
    merge-radius:
      item: 2.5
      exp: 3.0
    max-tnt-per-tick: {max_tnt}
    verbose: false
"#
    )
}

/// `bukkit.yml` (`bukkitYml`).
pub fn bukkit_yml(cfg: &ServerConfig) -> String {
    let aggr = aggressive(cfg);
    let monsters = if aggr { 50 } else { 70 };
    let monster_spawns = if aggr { 2 } else { 1 };
    format!(
        r#"settings:
  warn-on-overload: true
spawn-limits:
  monsters: {monsters}
  animals: 10
  water-animals: 5
  water-ambient: 20
  ambient: 15
ticks-per:
  monster-spawns: {monster_spawns}
  animal-spawns: 400
  autosave: 6000
chunk-gc:
  period-in-ticks: 600
"#
    )
}
