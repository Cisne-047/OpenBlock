//! El modo avanzado: las flags y la config que sale de ellas, portado de
//! `src/advanced.ts`.
//!
//! Es la puerta de entrada del CLI: `--engine`, `--mc`, `--ram-mb`, `--custom`… y,
//! si no se dice nada, los valores que recomienda el hardware detectado. Lo que hay
//! detrás (plan, generador, apply, doctor) ya estaba portado y verificado.
//!
//! La precedencia es la del TS y importa: **flag > config previa > hardware**. Un
//! `--config` sirve de base para reimportar un server y cambiarle una cosa.

use forge_core::hw::tiers::{classify_hardware, recommended_ram_mb};
use forge_core::types::{
    BackupConfig, CustomItem, EngineId, ModpackKind, ModpackRef, PerfProfile, RconConfig,
    ServerConfig, ServiceConfig, TargetOs,
};
use std::path::Path;

/// Las flags del modo avanzado (`AdvancedArgs`).
#[derive(Debug, Clone, Default)]
pub struct ArgsAvanzados {
    pub config: Option<String>,
    pub name: Option<String>,
    pub mc: Option<String>,
    pub engine: Option<String>,
    pub ram_mb: Option<u32>,
    pub players: Option<u32>,
    pub dir: Option<String>,
    pub port: Option<u16>,
    pub vd: Option<u32>,
    pub sd: Option<u32>,
    pub profile: Option<String>,
    pub pregen: Option<u32>,
    pub modpack: Option<String>,
    pub build: Option<String>,
    pub loader_build: Option<String>,
    pub installer_build: Option<String>,
    pub custom: Option<Vec<String>>,
    pub no_service: bool,
    pub no_backup: bool,
    pub no_rcon: bool,
    pub no_pregen: bool,
    pub target: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
    pub force: bool,
    pub help: bool,
    /// de cada comando, no de la config: generate --zip y el numero de hilos
    pub zip: Option<String>,
    pub cores: Option<u32>,
}

/// Las flags que llevan valor, y qué hace cada una.
fn parsear_numero<T: std::str::FromStr>(valor: &str, flag: &str) -> Result<T, String> {
    valor
        .parse::<T>()
        .map_err(|_| format!("--{flag} no es un número válido: {valor}"))
}

/// Parsea las flags (`parseAdvancedArgs`).
///
/// Acepta `--flag valor` y `--flag=valor`, como `parseArgs` de Node. Una flag
/// desconocida **no** es un error: `strict: false` en el TS, para que el mismo argv
/// valga entre versiones.
pub fn parse_avanzados(argv: &[String]) -> Result<ArgsAvanzados, String> {
    let mut a = ArgsAvanzados::default();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        let (nombre, valor_en_linea) = match arg.strip_prefix("--") {
            Some(resto) => match resto.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (resto.to_string(), None),
            },
            None => {
                // un posicional suelto es el fichero de config, para no obligar a
                // escribir `--config` en el caso más común
                if a.config.is_none() {
                    a.config = Some(arg.clone());
                }
                i += 1;
                continue;
            }
        };
        let tomar_valor = |i: &mut usize| -> Result<String, String> {
            if let Some(v) = &valor_en_linea {
                return Ok(v.clone());
            }
            *i += 1;
            argv.get(*i)
                .cloned()
                .ok_or_else(|| format!("--{nombre} necesita un valor"))
        };
        match nombre.as_str() {
            "config" => a.config = Some(tomar_valor(&mut i)?),
            "name" => a.name = Some(tomar_valor(&mut i)?),
            "mc" => a.mc = Some(tomar_valor(&mut i)?),
            "engine" => a.engine = Some(tomar_valor(&mut i)?),
            "ram-mb" => a.ram_mb = Some(parsear_numero(&tomar_valor(&mut i)?, "ram-mb")?),
            "players" => a.players = Some(parsear_numero(&tomar_valor(&mut i)?, "players")?),
            "dir" => a.dir = Some(tomar_valor(&mut i)?),
            "port" => a.port = Some(parsear_numero(&tomar_valor(&mut i)?, "port")?),
            "vd" => a.vd = Some(parsear_numero(&tomar_valor(&mut i)?, "vd")?),
            "sd" => a.sd = Some(parsear_numero(&tomar_valor(&mut i)?, "sd")?),
            "profile" => a.profile = Some(tomar_valor(&mut i)?),
            "pregen" => a.pregen = Some(parsear_numero(&tomar_valor(&mut i)?, "pregen")?),
            "modpack" => a.modpack = Some(tomar_valor(&mut i)?),
            "build" => a.build = Some(tomar_valor(&mut i)?),
            "loader-build" => a.loader_build = Some(tomar_valor(&mut i)?),
            "installer-build" => a.installer_build = Some(tomar_valor(&mut i)?),
            "custom" => {
                let v = tomar_valor(&mut i)?;
                a.custom = Some(
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            "target" => a.target = Some(tomar_valor(&mut i)?),
            "no-service" => a.no_service = true,
            "no-backup" => a.no_backup = true,
            "no-rcon" => a.no_rcon = true,
            "no-pregen" => a.no_pregen = true,
            "zip" => a.zip = Some(tomar_valor(&mut i)?),
            "cores" => a.cores = Some(parsear_numero(&tomar_valor(&mut i)?, "cores")?),
            "dry-run" => a.dry_run = true,
            "yes" => a.yes = true,
            "force" => a.force = true,
            "help" => a.help = true,
            // `strict: false`: lo que no se conoce se ignora, como en el TS
            _ => {}
        }
        i += 1;
    }
    Ok(a)
}

/// Construye la config: flag > config previa > hardware (`buildAdvancedConfig`).
pub fn construir_config(a: &ArgsAvanzados) -> Result<ServerConfig, String> {
    let base: Option<ServerConfig> = match &a.config {
        Some(p) => {
            let txt = std::fs::read_to_string(p).map_err(|_| format!("No existe {p}"))?;
            Some(serde_json::from_str(&txt).map_err(|e| format!("{p}: {e}"))?)
        }
        None => None,
    };

    let hw = crate::detect::detect_hardware();
    let tier = classify_hardware(&hw);

    // la versión de MC se pide a Mojang si no la dice nadie
    let mc = match (&a.mc, base.as_ref().map(|b| b.mc_version.clone())) {
        (Some(m), _) => m.clone(),
        (None, Some(m)) => m,
        (None, None) => forge_api::mojang::latest_release()
            .map_err(|e| format!("no se pudo saber la última versión de Minecraft: {e}"))?
            .to_string(),
    };

    let engine = match &a.engine {
        Some(e) => EngineId::parse(e).ok_or_else(|| format!("engine desconocido: {e}"))?,
        None => base
            .as_ref()
            .map(|b| b.engine)
            .unwrap_or(tier.recommended_engine),
    };
    let name = a
        .name
        .clone()
        .or_else(|| base.as_ref().map(|b| b.name.clone()))
        .unwrap_or_else(|| "mi-server".to_string());

    let url_modpack = match (&a.modpack, base.as_ref().map(|b| &b.modpack)) {
        (Some(u), _) => u.clone(),
        (None, Some(m)) if m.kind != ModpackKind::None => m.url.clone(),
        _ => String::new(),
    };

    let dir = match (&a.dir, base.as_ref().map(|b| b.server_dir.clone())) {
        (Some(d), _) => d.clone(),
        (None, Some(d)) => d,
        // el default del TS no expande la `~`: es literal, y se reproduce
        (None, None) => {
            if cfg!(windows) {
                format!("F:\\Servers\\{name}")
            } else {
                format!("~/servers/{name}")
            }
        }
    };

    let b = base.as_ref();
    let cfg = ServerConfig {
        name: name.clone(),
        mc_version: mc.clone(),
        engine,
        server_dir: crate::resolver(&dir).to_string_lossy().to_string(),
        ram_mb: a
            .ram_mb
            .or(b.map(|x| x.ram_mb))
            .unwrap_or(recommended_ram_mb(&hw, true) as u32),
        players: a
            .players
            .or(b.map(|x| x.players))
            .unwrap_or(tier.max_players),
        view_distance: a
            .vd
            .or(b.map(|x| x.view_distance))
            .unwrap_or(tier.view_distance),
        simulation_distance: a
            .sd
            .or(b.map(|x| x.simulation_distance))
            .unwrap_or(tier.simulation_distance),
        port: a.port.or(b.map(|x| x.port)).unwrap_or(25565),
        motd: b.map(|x| x.motd.clone()).unwrap_or_else(|| {
            format!(
                "{engine} {mc} — forjado con OpenBlock",
                engine = engine.as_str()
            )
        }),
        difficulty: b.map(|x| x.difficulty).unwrap_or_default(),
        gamemode: b.map(|x| x.gamemode).unwrap_or_default(),
        world_type: b.map(|x| x.world_type).unwrap_or_default(),
        seed: b.map(|x| x.seed.clone()).unwrap_or_default(),
        online_mode: b.map(|x| x.online_mode).unwrap_or(true),
        whitelist: b.map(|x| x.whitelist).unwrap_or(false),
        pvp: b.map(|x| x.pvp).unwrap_or(true),
        hardcore: b.map(|x| x.hardcore).unwrap_or(false),
        modpack: if url_modpack.is_empty() {
            ModpackRef {
                kind: ModpackKind::None,
                url: String::new(),
            }
        } else {
            ModpackRef {
                kind: b.map(|x| x.modpack.kind).unwrap_or(ModpackKind::Modrinth),
                url: url_modpack,
            }
        },
        performance: match &a.profile {
            Some(p) => PerfProfile::parse(p).ok_or_else(|| format!("perfil desconocido: {p}"))?,
            None => b.map(|x| x.performance).unwrap_or_default(),
        },
        pregen_radius: a
            .pregen
            .or(b.map(|x| x.pregen_radius))
            .unwrap_or(tier.pregen_radius),
        backup: BackupConfig {
            enabled: !a.no_backup && b.map(|x| x.backup.enabled).unwrap_or(true),
            interval_hours: b.map(|x| x.backup.interval_hours).unwrap_or(24),
            keep: b.map(|x| x.backup.keep).unwrap_or(7),
        },
        service: ServiceConfig {
            enabled: !a.no_service && b.map(|x| x.service.enabled).unwrap_or(false),
            user: b.and_then(|x| x.service.user.clone()),
        },
        rcon: RconConfig {
            enabled: !a.no_rcon && b.map(|x| x.rcon.enabled).unwrap_or(true),
            password: b.map(|x| x.rcon.password.clone()).unwrap_or_default(),
            port: b.map(|x| x.rcon.port).unwrap_or(25575),
        },
        chunkyborder: b.map(|x| x.chunkyborder).unwrap_or(false),
        target_os: match &a.target {
            // `TargetOs` no tiene un `parse` en el núcleo porque nadie más lo
            // necesitaba; el TS castea el string, aquí se comprueba
            Some(t) => match t.as_str() {
                "linux" => TargetOs::Linux,
                "windows" => TargetOs::Windows,
                "both" => TargetOs::Both,
                _ => return Err(format!("target desconocido: {t}")),
            },
            None => b.map(|x| x.target_os).unwrap_or_default(),
        },
        build: a.build.clone().or_else(|| b.and_then(|x| x.build.clone())),
        loader_build: a
            .loader_build
            .clone()
            .or_else(|| b.and_then(|x| x.loader_build.clone())),
        installer_build: a
            .installer_build
            .clone()
            .or_else(|| b.and_then(|x| x.installer_build.clone())),
        custom_items: match &a.custom {
            Some(fuentes) => fuentes
                .iter()
                .map(|src| {
                    let nombre = Path::new(src)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let sin_jar = match nombre.to_lowercase().strip_suffix(".jar") {
                        Some(_) => nombre[..nombre.len() - 4].to_string(),
                        None => nombre,
                    };
                    let nombre = if sin_jar.is_empty() {
                        "custom".to_string()
                    } else {
                        sin_jar
                    };
                    let kind = match engine {
                        EngineId::Fabric
                        | EngineId::Quilt
                        | EngineId::Forge
                        | EngineId::Neoforge => "mod",
                        _ => "plugin",
                    };
                    let es_url = src.to_lowercase().starts_with("http://")
                        || src.to_lowercase().starts_with("https://");
                    CustomItem {
                        name: nombre,
                        kind: kind.to_string(),
                        url: if es_url { Some(src.clone()) } else { None },
                        path: if es_url {
                            None
                        } else {
                            Some(crate::resolver(src).to_string_lossy().to_string())
                        },
                    }
                })
                .collect(),
            None => b.map(|x| x.custom_items.clone()).unwrap_or_default(),
        },
        java_path: b.and_then(|x| x.java_path.clone()),
    };

    let mut cfg = cfg;
    // `ensureRconPassword`: con RCON y sin contraseña, una al azar
    if cfg.rcon.enabled && cfg.rcon.password.is_empty() {
        cfg.rcon.password = password_hex();
    }
    Ok(cfg)
}

/// La contraseña RCON que se genera sola (`ensureRconPassword`).
///
/// **No es la misma que la del doctor**, y no es un descuido: el TS tiene dos
/// generadores distintos. Aquí es `randomBytes(8).toString("hex")`, o sea 16
/// caracteres hexadecimales; el fix `rcon-password` del doctor usa
/// `Math.random().toString(36)`, que son 14 en base 36. Se descubrió comparando el
/// `server.properties` de los dos CLI, que es exactamente para lo que está el diff.
///
/// La entropía sale del sistema operativo (`getrandom`), no del reloj: el TS usa
/// `crypto.randomBytes` y una contraseña de RCON no es sitio para un xorshift.
fn password_hex() -> String {
    let mut bytes = [0u8; 8];
    if getrandom::getrandom(&mut bytes).is_err() {
        // sin entropía del sistema, el reloj: peor, pero no se deja el server sin
        // contraseña teniendo RCON habilitado
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        bytes = nanos.to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_flags_se_parsean_como_en_el_ts() {
        let args: Vec<String> = [
            "--engine",
            "fabric",
            "--mc=1.21.4",
            "--ram-mb",
            "4096",
            "--no-backup",
            "--custom",
            "a.jar, https://x/b.jar ,",
            "--desconocida",
            "lo-que-sea",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let a = parse_avanzados(&args).unwrap();
        assert_eq!(a.engine.as_deref(), Some("fabric"));
        // las dos formas de pasar el valor
        assert_eq!(a.mc.as_deref(), Some("1.21.4"));
        assert_eq!(a.ram_mb, Some(4096));
        assert!(a.no_backup);
        assert!(!a.no_service);
        // el `custom` se parte por comas y se recortan los huecos
        assert_eq!(a.custom.unwrap(), vec!["a.jar", "https://x/b.jar"]);
    }

    #[test]
    fn un_posicional_suelto_es_el_fichero_de_config() {
        let args: Vec<String> = vec![
            "cfg.json".to_string(),
            "--port".to_string(),
            "25570".to_string(),
        ];
        let a = parse_avanzados(&args).unwrap();
        assert_eq!(a.config.as_deref(), Some("cfg.json"));
        assert_eq!(a.port, Some(25570));
    }

    #[test]
    fn sin_config_ni_mc_la_version_sale_del_default_del_ts() {
        // sin `--mc` habría que preguntar a Mojang; con `--mc` no se toca la red
        let a = parse_avanzados(&["--mc".to_string(), "1.20.4".to_string()]).unwrap();
        let cfg = construir_config(&a).unwrap();
        assert_eq!(cfg.mc_version, "1.20.4");
        assert_eq!(cfg.name, "mi-server");
        assert_eq!(cfg.port, 25565);
        // el `motd` lleva el guion largo del TS, no un guion normal
        assert_eq!(
            cfg.motd,
            format!("{} 1.20.4 — forjado con OpenBlock", cfg.engine.as_str())
        );
        assert_eq!(cfg.modpack.kind, ModpackKind::None);
        // RCON habilitado por defecto, y con contraseña: el server no arranca sin ella
        assert!(cfg.rcon.enabled);
        assert!(!cfg.rcon.password.is_empty());
        assert!(forge_core::generators::validate(&cfg).is_empty());
    }

    #[test]
    fn el_custom_sale_con_url_o_con_ruta_y_su_kind() {
        let args = parse_avanzados(&[
            "--mc".to_string(),
            "1.20.4".to_string(),
            "--engine".to_string(),
            "fabric".to_string(),
            "--custom".to_string(),
            "https://x/Sodium.jar,./local/Mod.jar".to_string(),
        ])
        .unwrap();
        let cfg = construir_config(&args).unwrap();
        assert_eq!(cfg.custom_items.len(), 2);
        // fabric es engine de mods
        assert_eq!(cfg.custom_items[0].kind, "mod");
        assert_eq!(cfg.custom_items[0].name, "Sodium");
        assert_eq!(
            cfg.custom_items[0].url.as_deref(),
            Some("https://x/Sodium.jar")
        );
        assert!(cfg.custom_items[0].path.is_none());
        assert_eq!(cfg.custom_items[1].name, "Mod");
        assert!(cfg.custom_items[1].url.is_none());
        assert!(cfg.custom_items[1]
            .path
            .as_deref()
            .unwrap()
            .ends_with("Mod.jar"));
    }
}
