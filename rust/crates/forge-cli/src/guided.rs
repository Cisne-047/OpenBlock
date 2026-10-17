//! El modo guiado: el asistente que pregunta, portado de `src/guided.ts` y de los
//! prompts de `src/ui.ts`.
//!
//! Las preguntas leen de la entrada estándar **línea a línea**, igual que
//! `readline` en el TS, con el mismo menú numerado y los mismos valores por defecto:
//! una línea vacía toma el default. Eso es lo que permite comparar los dos
//! asistentes alimentándolos con las mismas respuestas.
//!
//! Lo que **no** está portado: la rama del modpack (Modrinth y CurseForge). Pide la
//! URL, resuelve el `.mrpack` y saca de ahí la versión de MC y el loader; se dice y
//! se sale, en vez de fingir que se ha hecho.

use forge_core::types::{
    BackupConfig, CustomItem, EngineId, ModpackKind, ModpackRef, PerfProfile, RconConfig,
    ServerConfig, ServiceConfig, TargetOs,
};
use std::io::{BufRead, Write};

/// Pregunta una línea, con su valor por defecto (`ask`).
fn ask(pregunta: &str, por_defecto: &str) -> Result<String, String> {
    print!("{pregunta} [{por_defecto}]: ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut linea = String::new();
    let leidos = std::io::stdin()
        .lock()
        .read_line(&mut linea)
        .map_err(|e| e.to_string())?;
    if leidos == 0 {
        // sin entrada (un pipe que se acaba): se toma el default, como si el usuario
        // hubiera pulsado Enter
        return Ok(por_defecto.to_string());
    }
    let limpia = linea.trim();
    Ok(if limpia.is_empty() {
        por_defecto.to_string()
    } else {
        limpia.to_string()
    })
}

/// Pregunta un número, con default (`askNumber`).
///
/// Lo que no sea un número deja el default y avisa, en vez de reventar: en el TS
/// `Number("abc")` da `NaN` y se cuela en la config.
fn ask_number(pregunta: &str, por_defecto: u32) -> Result<u32, String> {
    let raw = ask(pregunta, &por_defecto.to_string())?;
    match raw.trim().parse::<u32>() {
        Ok(n) => Ok(n),
        Err(_) => {
            println!("  No es un número: me quedo con {por_defecto}.");
            Ok(por_defecto)
        }
    }
}

/// Elige entre opciones por número o por id (`askChoice`), y repite si no vale.
fn ask_choice(
    pregunta: &str,
    opciones: &[(&str, &str, &str)],
    por_defecto: &str,
) -> Result<String, String> {
    println!();
    for (i, (_, label, hint)) in opciones.iter().enumerate() {
        if hint.is_empty() {
            println!("  {:>2}. {label}", i + 1);
        } else {
            println!("  {:>2}. {label} — {hint}", i + 1);
        }
    }
    loop {
        let raw = ask(pregunta, por_defecto)?;
        if let Ok(n) = raw.parse::<usize>() {
            if n >= 1 && n <= opciones.len() {
                return Ok(opciones[n - 1].0.to_string());
            }
        }
        let minuscula = raw.to_lowercase();
        if let Some((id, _, _)) = opciones.iter().find(|(id, _, _)| *id == minuscula) {
            return Ok(id.to_string());
        }
        println!("  Opción inválida.");
    }
}

/// Pregunta de sí o no (`confirm`).
fn confirm(pregunta: &str, por_defecto: bool) -> Result<bool, String> {
    let raw = ask(
        &format!("{pregunta} (s/n)"),
        if por_defecto { "s" } else { "n" },
    )?;
    Ok(matches!(
        raw.to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    ))
}

/// El asistente (`guided`).
pub fn guided() -> Result<(), String> {
    let hw = crate::detect::detect_hardware();
    let tier = forge_core::hw::tiers::classify_hardware(&hw);

    println!("\n== Tu hardware ==");
    for (k, v) in [
        ("CPU", format!("{} ({} hilos)", hw.cpu_model, hw.cores)),
        (
            "RAM",
            format!(
                "{:.1} GB (libres {:.1} GB)",
                hw.ram_total_mb as f64 / 1024.0,
                hw.ram_free_mb as f64 / 1024.0
            ),
        ),
        ("Disco", format!("{:?}", hw.disk_kind).to_uppercase()),
        ("Sistema", format!("{} {}", hw.os, hw.os_version)),
        (
            "Tier",
            format!(
                "{} — {} jugadores, vd {}/{}",
                tier.label, tier.max_players, tier.view_distance, tier.simulation_distance
            ),
        ),
    ] {
        println!("  {k:<10} {v}");
    }
    println!("\n  Tip: `forge bench` mide CPU/disco de verdad y afina la recomendación.");

    let dedicada = ask_choice(
        "\n¿Para qué es esta máquina?",
        &[
            (
                "dedicada",
                "Solo el server (dedicada)",
                "usa hasta el 70-80% de la RAM",
            ),
            ("compartida", "La uso para más cosas", "usa ~45% de la RAM"),
        ],
        "dedicada",
    )?;

    let modo = ask_choice(
        "¿Qué server quieres montar?",
        &[
            (
                "facil",
                "Súper fácil",
                "recomendado por tu hardware, todo automático",
            ),
            (
                "plugins",
                "Paper con plugins de rendimiento",
                "el estándar para jugar con amigos",
            ),
            (
                "modpack",
                "Modpack",
                "URL de Modrinth o zip exportado de CurseForge",
            ),
            ("modded", "Modded a mano (Fabric/Forge/NeoForge)", ""),
            (
                "vanilla",
                "Vanilla puro",
                "sin optimizaciones, tal cual Mojang",
            ),
        ],
        "facil",
    )?;

    let engine: EngineId;
    let mc_version: String;
    let modpack = ModpackRef {
        kind: ModpackKind::None,
        url: String::new(),
    };
    let mut build: Option<String> = None;

    match modo.as_str() {
        "facil" => {
            engine = tier.recommended_engine;
            mc_version = forge_api::mojang::latest_release()
                .map_err(|e| e.to_string())?
                .to_string();
            println!(
                "  → {} {mc_version}: lo óptimo para tu máquina.",
                forge_core::catalog::engine_by_id(engine).label
            );
        }
        "vanilla" => {
            engine = EngineId::Vanilla;
            let ultima = forge_api::mojang::latest_release()
                .map_err(|e| e.to_string())?
                .to_string();
            mc_version = ask("Versión de MC", &ultima)?;
        }
        "plugins" | "modded" => {
            let (titulo, solo_plugins) = if modo == "plugins" {
                ("Engine", true)
            } else {
                ("Loader", false)
            };
            let opciones: Vec<(String, String, String)> = forge_core::catalog::ENGINES
                .iter()
                .filter(|e| e.plugin_api == solo_plugins && (solo_plugins || e.mod_api))
                .map(|e| {
                    (
                        e.id.as_str().to_string(),
                        e.label.to_string(),
                        e.desc.to_string(),
                    )
                })
                .collect();
            let refs: Vec<(&str, &str, &str)> = opciones
                .iter()
                .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
                .collect();
            let elegido = ask_choice(
                titulo,
                &refs,
                if modo == "plugins" {
                    tier.recommended_engine.as_str()
                } else {
                    "fabric"
                },
            )?;
            engine = EngineId::parse(&elegido)
                .ok_or_else(|| format!("engine desconocido: {elegido}"))?;
            let ultima = forge_api::mojang::latest_release()
                .map_err(|e| e.to_string())?
                .to_string();
            mc_version = ask("Versión de MC", &ultima)?;
            build = elegir_build(engine, &mc_version)?;
        }
        "modpack" => {
            return Err(
                "El modpack del modo guiado todavía no está portado: usa `forge generate --modpack <url>` o el CLI de TypeScript."
                    .to_string(),
            );
        }
        otro => return Err(format!("modo desconocido: {otro}")),
    }

    let ram_defecto = forge_core::hw::tiers::recommended_ram_mb(&hw, dedicada == "dedicada") as u32;
    let ram_mb = ask_number("RAM para el server (MB)", ram_defecto)?;
    let players = ask_number("Jugadores", tier.max_players)?;
    let name = ask("Nombre del server", "mi-server")?;
    let port = ask_number("Puerto", 25565)?;
    let motd = ask(
        "MOTD",
        &format!(
            "{} {mc_version} — forjado con OpenBlock",
            forge_core::catalog::engine_by_id(engine).label
        ),
    )?;
    let pregen_radius = ask_number(
        "Pregenerar mundo (radio en bloques, 0 = no)",
        if tier.pregen_radius == 0 {
            2000
        } else {
            tier.pregen_radius
        },
    )?;
    let perfil = ask_choice(
        "Perfil de rendimiento",
        &[
            ("conservative", "Conservador", "toca lo mínimo"),
            ("balanced", "Equilibrado", "lo seguro, sin tocar gameplay"),
            (
                "aggressive",
                "Agresivo",
                "más TPS, rangos de mobs reducidos",
            ),
        ],
        "balanced",
    )?;
    let service_enabled = confirm("¿Autoarranque como servicio del sistema?", false)?;
    let backup_enabled = confirm("¿Backups automáticos (diarios, 7 copias)?", true)?;

    // plugins/mods propios, por URL o ruta local
    let mut custom_items: Vec<CustomItem> = Vec::new();
    while confirm("¿Añadir un plugin/mod propio (URL o ruta local)?", false)? {
        let kind = ask_choice(
            "¿Qué es?",
            &[
                ("plugin", "Plugin (plugins/, engines Paper-like)", ""),
                ("mod", "Mod (mods/, loaders de mods)", ""),
            ],
            "plugin",
        )?;
        let src = ask("URL directa del .jar o ruta local", "")?;
        if src.is_empty() {
            continue;
        }
        let nombre = match std::path::Path::new(&src).file_name() {
            Some(f) => {
                let f = f.to_string_lossy().to_string();
                match f.to_lowercase().strip_suffix(".jar") {
                    Some(_) => f[..f.len() - 4].to_string(),
                    None => f,
                }
            }
            None => "custom".to_string(),
        };
        let nombre = if nombre.is_empty() {
            "custom".to_string()
        } else {
            nombre
        };
        let es_url =
            src.to_lowercase().starts_with("http://") || src.to_lowercase().starts_with("https://");
        custom_items.push(CustomItem {
            name: nombre.clone(),
            kind,
            url: if es_url { Some(src.clone()) } else { None },
            path: if es_url {
                None
            } else {
                Some(crate::resolver(&src).to_string_lossy().to_string())
            },
        });
        println!("  ✓ {nombre} añadido");
    }

    let dir_defecto = if cfg!(windows) {
        format!("F:\\Servers\\{name}")
    } else {
        format!("~/servers/{name}")
    };
    let server_dir = ask("Directorio del server", &dir_defecto)?;
    // `~` al principio se expande con HOME, como en el TS
    let server_dir = match server_dir.strip_prefix('~') {
        Some(resto) if resto.is_empty() || resto.starts_with('/') => {
            format!("{}{resto}", std::env::var("HOME").unwrap_or_default())
        }
        _ => server_dir,
    };

    let mut cfg = ServerConfig {
        name,
        mc_version,
        engine,
        server_dir: crate::resolver(&server_dir).to_string_lossy().to_string(),
        ram_mb,
        players,
        view_distance: tier.view_distance,
        simulation_distance: tier.simulation_distance,
        port: u16::try_from(port).map_err(|_| format!("puerto fuera de rango: {port}"))?,
        motd,
        difficulty: Default::default(),
        gamemode: Default::default(),
        world_type: Default::default(),
        seed: String::new(),
        online_mode: true,
        whitelist: false,
        pvp: true,
        hardcore: false,
        modpack,
        performance: PerfProfile::parse(&perfil)
            .ok_or_else(|| format!("perfil desconocido: {perfil}"))?,
        pregen_radius,
        backup: BackupConfig {
            enabled: backup_enabled,
            interval_hours: 24,
            keep: 7,
        },
        service: ServiceConfig {
            enabled: service_enabled,
            user: None,
        },
        rcon: RconConfig {
            // RCON solo si hay pregeneración, que es lo que lo necesita
            enabled: pregen_radius > 0,
            password: crate::advanced::password_hex_publico(),
            port: 25575,
        },
        chunkyborder: false,
        target_os: if cfg!(windows) {
            TargetOs::Windows
        } else {
            TargetOs::Linux
        },
        build,
        loader_build: None,
        installer_build: None,
        custom_items,
        java_path: None,
    };
    if cfg.rcon.enabled && cfg.rcon.password.is_empty() {
        cfg.rcon.password = crate::advanced::password_hex_publico();
    }

    println!("\n== Config final ==");
    let custom = if cfg.custom_items.is_empty() {
        "nada".to_string()
    } else {
        cfg.custom_items
            .iter()
            .map(|i| i.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (k, v) in [
        (
            "Engine",
            format!(
                "{} {}{}",
                forge_core::catalog::engine_by_id(cfg.engine).label,
                cfg.mc_version,
                match &cfg.build {
                    Some(b) => format!(" (build {b})"),
                    None => String::new(),
                }
            ),
        ),
        ("RAM", format!("{} MB", cfg.ram_mb)),
        ("Jugadores", cfg.players.to_string()),
        (
            "Distancias",
            format!(
                "view {} / sim {}",
                cfg.view_distance, cfg.simulation_distance
            ),
        ),
        ("Perfil", cfg.performance.as_str().to_string()),
        (
            "Pregen",
            if cfg.pregen_radius > 0 {
                format!("{} bloques (Chunky + RCON)", cfg.pregen_radius)
            } else {
                "off".to_string()
            },
        ),
        ("Custom", custom),
        (
            "Servicio",
            if cfg.service.enabled { "sí" } else { "no" }.to_string(),
        ),
        (
            "Backups",
            if cfg.backup.enabled {
                format!(
                    "cada {}h, {} copias",
                    cfg.backup.interval_hours, cfg.backup.keep
                )
            } else {
                "no".to_string()
            },
        ),
        ("Directorio", cfg.server_dir.clone()),
    ] {
        println!("  {k:<10} {v}");
    }

    if !confirm(
        "\n¿Instalamos? (baja el engine, plugins/mods y deja el server listo)",
        true,
    )? {
        println!("Cancelado. Tu config queda en la pregunta anterior; nada se ha tocado.");
        return Ok(());
    }

    let opts = crate::apply::OpcionesApply {
        cores: hw.cores,
        ..Default::default()
    };
    crate::apply::apply_config(&mut cfg, &opts, &mut |l| println!("{l}"))?;
    if cfg.pregen_radius > 0 {
        let dir = crate::resolver(&cfg.server_dir);
        let (plan, _) = forge_api::plan::build_download_plan(&cfg).map_err(|e| e.to_string())?;
        let java_path = std::fs::read_to_string(dir.join("runtime").join("java.path"))
            .map(|t| t.trim().to_string())
            .unwrap_or_else(|_| "java".to_string());
        crate::apply::pregenerate(&cfg, &dir, &java_path, &plan, &mut |l| println!("{l}"))?;
    }
    Ok(())
}

/// Deja elegir una build concreta del engine (`pickBuild`).
///
/// Si la API no da builds, se sigue sin build: es lo que hace el TS cuando el
/// usuario pulsa Enter.
fn elegir_build(engine: EngineId, mc: &str) -> Result<Option<String>, String> {
    let builds = builds_for(engine, mc)?;
    if builds.is_empty() {
        return Ok(None);
    }
    println!(
        "\n  Builds disponibles de {} para {mc}: {} (las 10 primeras)",
        forge_core::catalog::engine_by_id(engine).label,
        builds.len()
    );
    for b in builds.iter().take(10) {
        println!("    {b}");
    }
    let elegida = ask("Build (Enter = la última)", "")?;
    Ok(if elegida.is_empty() {
        None
    } else {
        Some(elegida)
    })
}

/// Las builds que ofrece un engine para una versión (`buildsFor`).
fn builds_for(engine: EngineId, mc: &str) -> Result<Vec<String>, String> {
    use forge_core::types::EngineId as E;
    let lista = match engine {
        E::Paper | E::Folia => {
            let project = if engine == E::Folia { "folia" } else { "paper" };
            forge_api::engines::paper_builds(project, mc)
        }
        E::Purpur => forge_api::engines::purpur_builds(mc).map_err(|e| e.to_string())?,
        E::Fabric => forge_api::engines::fabric_loader_versions(mc).map_err(|e| e.to_string())?,
        E::Quilt => forge_api::engines::quilt_loader_versions(mc).map_err(|e| e.to_string())?,
        E::Forge => forge_api::engines::forge_builds(mc).map_err(|e| e.to_string())?,
        E::Neoforge => forge_api::engines::neoforge_builds(mc).map_err(|e| e.to_string())?,
        E::Vanilla => Vec::new(),
    };
    Ok(lista.into_iter().map(|b| b.id_str()).collect())
}
