//! Generadores puros: cada módulo construye contenido y nada más.
//!
//! Mismo contrato que el TS: `fn(cfg) -> String` para los generadores de hoja.
//! Ninguno toca el disco, la red ni el reloj. Este módulo es el port de
//! `src/generators/index.ts`, incluido `generateProject`.

pub mod jvm;
pub mod ops;
pub mod paper;
pub mod properties;
pub mod scripts;
pub mod service;

use crate::catalog::{engine_by_id, java_major_for};
use crate::plan::DownloadPlan;
use crate::types::{EngineId, ServerConfig, TargetOs};
use crate::FileMap;

/// `server.properties` (alias del módulo, para que la llamada se lea como en TS).
pub fn server_properties(cfg: &ServerConfig) -> String {
    properties::server_properties(cfg)
}

/// GC que toca según RAM y cores (`generateProject` lo decide así en el TS).
///
/// Ojo: es la decisión del generador, no la de `recommendedGc` de `hw/tiers`
/// —el TS tiene la misma duplicidad a propósito, con entradas distintas—.
pub fn gc_for(ram_mb: u32, cores: u32) -> &'static str {
    if ram_mb >= 16 * 1024 && cores >= 8 {
        "zgc"
    } else {
        "g1"
    }
}

/// Valida la config y devuelve los problemas en texto (`validate`).
///
/// DistroForge usa el mismo modelo —`Vec<String>` de mensajes para el usuario, sin
/// códigos ni severidades— y el TS de OpenBlock hace lo mismo desde el principio.
/// Es una simplificación deliberada: todo lo que devuelve bloquea.
pub fn validate(cfg: &ServerConfig) -> Vec<String> {
    let mut out = Vec::new();
    if !nombre_valido(&cfg.name) {
        out.push("Nombre: minúsculas, números y guiones (2-32 caracteres).".to_string());
    }
    if cfg.port < 1024 {
        out.push("Puerto fuera de rango (1024-65535).".to_string());
    }
    if cfg.ram_mb < 512 {
        out.push("RAM asignada demasiado baja (mínimo 512 MB).".to_string());
    }
    if cfg.players < 1 || cfg.players > 1000 {
        out.push("Jugadores: 1-1000.".to_string());
    }
    if cfg.view_distance < 3 || cfg.view_distance > 32 {
        out.push("view-distance: 3-32.".to_string());
    }
    if cfg.simulation_distance < 5 || cfg.simulation_distance > 32 {
        out.push("simulation-distance: 5-32.".to_string());
    }
    // pregenRadius es u32, así que "negativo" no existe: el TS lo comprueba
    // porque su número puede serlo. Es una divergencia de tipo, no de conducta.
    if cfg.rcon.enabled && cfg.rcon.password.chars().count() < 8 {
        out.push("RCON habilitado: contraseña de al menos 8 caracteres.".to_string());
    }
    if cfg.motd.is_empty() {
        out.push("MOTD vacío.".to_string());
    }
    out
}

/// `/^[a-z0-9][a-z0-9_-]{1,31}$/`: el TS lo comprueba con regex, aquí a mano.
fn nombre_valido(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(primero) = chars.next() else {
        return false;
    };
    if !(primero.is_ascii_lowercase() || primero.is_ascii_digit()) {
        return false;
    }
    let resto: Vec<char> = chars.collect();
    // `{1,31}` del regex: tras el primer carácter quedan entre 1 y 31
    if resto.is_empty() || resto.len() > 31 {
        return false;
    }
    resto
        .iter()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-')
}

/// README del proyecto generado (`readmeMd`).
pub fn readme_md(cfg: &ServerConfig, plan: &DownloadPlan) -> String {
    let engine = engine_by_id(cfg.engine);
    let items: Vec<String> = plan
        .entries
        .iter()
        .filter(|e| {
            e.kind == crate::plan::DownloadKind::Plugin || e.kind == crate::plan::DownloadKind::Mod
        })
        .map(|e| format!("- {} ({})", e.label, e.kind.as_str()))
        .collect();
    let resolved = if plan.engine_resolved != cfg.mc_version {
        format!("({})", plan.engine_resolved)
    } else {
        String::new()
    };
    // el TS interpola `items.length ? "\n### …\n" : ""`: el salto de línea inicial
    // va DENTRO del condicional, así que sin plugins no queda línea en blanco
    let bloque_items = if items.is_empty() {
        String::new()
    } else {
        format!(
            "\n### Plugins/mods de rendimiento\n\n{}\n",
            items.join("\n")
        )
    };
    let (name, mc, ram, port, players, vd, sd, perf, keep) = (
        &cfg.name,
        &cfg.mc_version,
        cfg.ram_mb,
        cfg.port,
        cfg.players,
        cfg.view_distance,
        cfg.simulation_distance,
        cfg.performance,
        cfg.backup.keep,
    );
    let label = engine.label;
    let desc = engine.desc;
    format!(
        r#"# {name}

Server de Minecraft forjado con **OpenBlock** — {label} {mc}
{resolved}

## Arrancar

- **Windows:** `start.bat`
- **Linux:** `./start.sh`

Si copiaste este proyecto a otra máquina sin instalar nada, ejecuta antes:

- **Windows:** `install.ps1` (PowerShell)
- **Linux:** `./install.sh`

## Qué lleva dentro

- Engine: {label} ({desc})
- RAM asignada: {ram} MB — flags: `jvm-args.txt`
- Puerto: {port} — jugadores: {players} — view-distance: {vd} / simulation: {sd}
- Perfil de rendimiento: {perf}
{bloque_items}
## Servicio

- **Linux:** `sudo ./install-service.sh` (systemd) — `systemctl status openblock-{name}`
- **Windows:** `install-service.ps1` como Administrador (WinSW)

## Firewall

- **Linux:** `sudo ./firewall.sh`
- **Windows:** `firewall.ps1` como Administrador

## Backups

- Manual: `./backup.sh` / `backup.ps1` — guarda mundos + configs en `backups/`, rota {keep}.
- Linux: instala el timer con `sudo cp openblock-backup.timer /etc/systemd/system/ && sudo systemctl enable --now openblock-backup.timer`.

## Config round-trip

Este proyecto lleva `openblock.json`: reimporta la config en OpenBlock o
regenera el server tras cambiarla.
"#
    )
}

/// Proyecto completo del server (`generateProject`).
///
/// Mismos ficheros que `apply` escribe en disco. `target_os` controla qué scripts
/// van (por defecto, ambos).
///
/// Este módulo y todo `generators/` es **puro**: no toca el disco ni la red, y no
/// lanza. La E/S vive en el CLI.
pub fn generate_project(cfg: &ServerConfig, plan: &DownloadPlan, cores: u32) -> FileMap {
    let engine = engine_by_id(cfg.engine);
    let gc = gc_for(cfg.ram_mb, cores);
    let args_line = jvm::jvm_args_line(cfg, gc, cores);
    let mut files = FileMap::new();

    files.insert("openblock.json".into(), cfg.to_json());
    files.insert("eula.txt".into(), "eula=true\n".into());
    files.insert(
        "server.properties".into(),
        properties::server_properties(cfg),
    );
    files.insert("start.sh".into(), scripts::start_sh(cfg, plan));
    files.insert("start.bat".into(), scripts::start_bat(cfg, plan));
    files.insert("jvm-args.txt".into(), scripts::jvm_args_txt(&args_line));
    files.insert("README.md".into(), readme_md(cfg, plan));

    if cfg.target_os != TargetOs::Windows {
        files.insert("install.sh".into(), scripts::install_sh(cfg, plan));
        files.insert("firewall.sh".into(), ops::firewall_sh(cfg));
        files.insert("backup.sh".into(), ops::backup_sh(cfg));
    }
    if cfg.target_os != TargetOs::Linux {
        files.insert("install.ps1".into(), scripts::install_ps1(cfg, plan));
        files.insert("firewall.ps1".into(), ops::firewall_ps1(cfg));
        files.insert("backup.ps1".into(), ops::backup_ps1(cfg));
    }
    if cfg.service.enabled {
        let user = cfg
            .service
            .user
            .clone()
            .unwrap_or_else(|| "minecraft".to_string());
        if cfg.target_os != TargetOs::Windows {
            files.insert(
                "openblock.service".into(),
                service::systemd_unit(cfg, &user),
            );
            files.insert(
                "install-service.sh".into(),
                service::install_service_sh(cfg, &user),
            );
        }
        if cfg.target_os != TargetOs::Linux {
            files.insert("winsw.xml".into(), service::winsw_config(cfg));
            files.insert(
                "install-service.ps1".into(),
                service::install_service_ps1(cfg),
            );
            files.insert(
                "remove-service.ps1".into(),
                service::remove_service_ps1(cfg),
            );
        }
    }
    if cfg.backup.enabled && cfg.target_os != TargetOs::Windows {
        files.insert("openblock-backup.timer".into(), ops::backup_timer(cfg));
    }
    // familia paper: configs dedicadas
    if engine.plugin_api {
        files.insert("paper-global.yml".into(), paper::paper_global_yml());
        files.insert(
            "paper-world-defaults.yml".into(),
            paper::paper_world_defaults_yml(cfg),
        );
        files.insert("spigot.yml".into(), paper::spigot_yml(cfg));
        files.insert("bukkit.yml".into(), paper::bukkit_yml(cfg));
    }
    // forge/neoforge: el launcher del installer lee las flags de aquí
    if engine.installer {
        files.insert(
            "user_jvm_args.txt".into(),
            scripts::user_jvm_args_txt(&args_line),
        );
    }
    files
}

/// Versión de Java que necesita el proyecto, para quien lo consuma desde fuera.
pub fn java_major_for_project(cfg: &ServerConfig) -> u32 {
    java_major_for(&cfg.mc_version)
}

/// Motores que llevan installer (Forge/NeoForge), útil para el CLI.
pub fn engine_needs_installer(id: EngineId) -> bool {
    engine_by_id(id).installer
}
