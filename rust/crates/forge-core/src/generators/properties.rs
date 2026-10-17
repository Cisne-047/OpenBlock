//! `server.properties` tuneado, portado de `src/generators/properties.ts`.

use crate::types::ServerConfig;

/// Escapa un valor de propiedad: barras y saltos de línea (`escPropValue`).
fn esc_prop_value(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\n', "\\n")
}

/// `server.properties` completo con los valores tuned.
///
/// `sync-chunk-writes=false` y `pause-when-empty-seconds` son las dos victorias
/// gratis del vanilla moderno. Las claves van en el mismo orden que el TS.
pub fn server_properties(cfg: &ServerConfig) -> String {
    let aggressive = cfg.performance == crate::types::PerfProfile::Aggressive;
    let lines: Vec<(&str, String)> = vec![
        ("accepts-transfers", "false".into()),
        ("allow-flight", "false".into()),
        ("allow-nether", "true".into()),
        ("broadcast-console-to-ops", "true".into()),
        ("broadcast-rcon-to-ops", "true".into()),
        ("difficulty", cfg.difficulty.as_str().into()),
        ("enable-command-block", "false".into()),
        ("enable-jmx-monitoring", "false".into()),
        ("enable-query", "false".into()),
        ("enable-rcon", cfg.rcon.enabled.to_string()),
        ("enable-status", "true".into()),
        ("enforce-secure-profile", "true".into()),
        ("enforce-whitelist", cfg.whitelist.to_string()),
        // rango de broadcast: 75% en aggressive reduce paquetes sin notarse
        (
            "entity-broadcast-range-percentage",
            if aggressive { "75" } else { "100" }.into(),
        ),
        ("force-gamemode", "false".into()),
        ("function-permission-level", "2".into()),
        ("gamemode", cfg.gamemode.as_str().into()),
        (
            "generate-structures",
            (cfg.world_type != crate::types::WorldType::Flat).to_string(),
        ),
        ("hardcore", cfg.hardcore.to_string()),
        ("hide-online-players", "false".into()),
        ("initial-disabled-packs", String::new()),
        ("initial-enabled-packs", "vanilla".into()),
        ("level-name", "world".into()),
        ("level-seed", cfg.seed.clone()),
        (
            "level-type",
            format!("minecraft\\:{}", cfg.world_type.as_str()),
        ),
        ("log-ips", "true".into()),
        (
            "max-chunk-sends-per-tick",
            if aggressive { "16" } else { "24" }.into(),
        ),
        (
            "max-chunk-receives-per-tick",
            if aggressive { "8" } else { "12" }.into(),
        ),
        ("max-players", cfg.players.to_string()),
        ("max-tick-time", "60000".into()),
        ("max-world-size", "29999984".into()),
        ("motd", esc_prop_value(&cfg.motd)),
        (
            "network-compression-threshold",
            if aggressive { "512" } else { "256" }.into(),
        ),
        ("online-mode", cfg.online_mode.to_string()),
        ("op-permission-level", "4".into()),
        // pausa el tick interno cuando no queda nadie: ahorra CPU y disco
        (
            "pause-when-empty-seconds",
            if aggressive { "30" } else { "60" }.into(),
        ),
        ("player-idle-timeout", "0".into()),
        ("prevent-proxy-connections", "false".into()),
        ("pvp", cfg.pvp.to_string()),
        ("query.port", cfg.port.to_string()),
        ("rate-limit", "0".into()),
        ("rcon.password", cfg.rcon.password.clone()),
        ("rcon.port", cfg.rcon.port.to_string()),
        ("region-file-compression", "deflate".into()),
        ("require-resource-pack", "false".into()),
        ("resource-pack", String::new()),
        ("resource-pack-prompt", String::new()),
        ("resource-pack-sha1", String::new()),
        ("server-ip", String::new()),
        ("server-port", cfg.port.to_string()),
        ("simulation-distance", cfg.simulation_distance.to_string()),
        ("spawn-monsters", "true".into()),
        ("spawn-protection", "16".into()),
        // IO síncrono de chunks: la mayor regresión de rendimiento en discos lentos
        ("sync-chunk-writes", "false".into()),
        ("text-filtering-config", String::new()),
        ("text-filtering-version", "0".into()),
        ("use-native-transport", "true".into()),
        ("view-distance", cfg.view_distance.to_string()),
        ("white-list", cfg.whitelist.to_string()),
    ];
    let mut out = lines
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}
