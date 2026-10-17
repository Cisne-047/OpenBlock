//! CLI de OpenBlock en Rust.
//!
//! El núcleo (`forge-core`) construye contenido y no toca el sistema; aquí vive
//! todo lo que sí lo toca: la detección de hardware y la escritura a disco. Es el
//! mismo reparto que el TS hace con `output.ts`.
//!
//! Subcomandos:
//!
//! - `forge detect [--json]` — hardware detectado, con el mismo JSON que el TS.
//! - `forge plan <config.json> [plan.json]` — resuelve el manifiesto contra las APIs.
//! - `forge dump <config.json> <plan.json> <dir> [cores]` — proyecto a disco.
//! - `forge doctor [--dir D] [--json]` — diagnostica un server instalado.
//!
//! En construcción: la pregeneración de install y el modo guiado.

mod apply;
mod detect;
mod doctor;
mod java;
mod rcon;
mod selftest;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn uso() -> ExitCode {
    eprintln!("uso: forge <comando> [opciones]");
    eprintln!();
    eprintln!("  detect [--json]                 hardware detectado (JSON igual que el TS)");
    eprintln!(
        "  plan <config> [plan.json]       resuelve el plan contra las APIs (sin -o, a stdout)"
    );
    eprintln!("  dump <config> <plan> <dir> [n]  genera el proyecto desde config y plan");
    eprintln!("  install <config> [--force]      resuelve el plan y baja todo: server instalado");
    eprintln!(
        "  generate <config> [--zip Z]     genera el proyecto (y el zip portable si se pide)"
    );
    eprintln!("  backup <dir>                    ejecuta el script de backup del server");
    eprintln!(
        "  stop <dir>                      apaga el server por RCON (tiene que estar arriba)"
    );
    eprintln!(
        "  self-test [--mc X] [--port N]   instala, arranca y diagnostica (es el smoke de CI)"
    );
    eprintln!("  doctor [--dir D] [--json]       diagnostica un server instalado (solo lectura)");
    eprintln!("  latest                          última release de Minecraft (consulta Mojang)");
    eprintln!("  rcon <host> <puerto> <comando>  manda un comando por RCON (pass: OPENBLOCK_RCON_PASSWORD)");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("detect") => detect_cmd(&args[1..]),
        Some("dump") => dump_cmd(&args[1..]),
        Some("install") => install_cmd(&args[1..]),
        Some("generate") => generate_cmd(&args[1..]),
        Some("backup") => backup_cmd(&args[1..]),
        Some("stop") => stop_cmd(&args[1..]),
        Some("self-test") => self_test_cmd(&args[1..]),
        Some("latest") => latest_cmd(),
        Some("doctor") => doctor_cmd(&args[1..]),
        Some("plan") => plan_cmd(&args[1..]),
        Some("rcon") => rcon_cmd(&args[1..]),
        _ => uso(),
    }
}

/// `forge rcon <host> <puerto> <comando>` — manda un comando a un server por RCON.
///
/// La contraseña se lee de `OPENBLOCK_RCON_PASSWORD` y **no** de un argumento: en
/// la línea de comandos quedaría visible en el listado de procesos de la máquina.
/// Es la misma razón por la que el doctor la lee de `openblock.json`.
fn rcon_cmd(args: &[String]) -> ExitCode {
    if args.len() < 3 {
        eprintln!("uso: forge rcon <host> <puerto> <comando>");
        eprintln!("  la contraseña se lee de OPENBLOCK_RCON_PASSWORD");
        return ExitCode::from(2);
    }
    let puerto: u16 = match args[1].parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Puerto inválido: {}", args[1]);
            return ExitCode::from(2);
        }
    };
    let comando = args[2..].join(" ");
    let Ok(password) = std::env::var("OPENBLOCK_RCON_PASSWORD") else {
        eprintln!("Falta OPENBLOCK_RCON_PASSWORD: la contraseña no se pasa por argumento");
        return ExitCode::from(2);
    };

    match rcon::with_rcon(&args[0], puerto, &password, |r| r.command(&comando)) {
        Ok(salida) => {
            println!("{salida}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Última release de Minecraft según Mojang (`latestRelease` del TS).
///
/// Existe para verificar la ruta de red de punta a punta: es lo único del puerto
/// que no se puede comparar con un golden, así que se comprueba que responde y que
/// el manifiesto se cachea.
/// `forge plan <config> [plan.json]`.
///
/// Este es el subcomando que hace verificable la parte que **no** puede tener
/// golden: se ejecuta contra las mismas APIs que el TS y se comparan los dos
/// `plan.json`. Si los dos resuelven en el mismo minuto, tienen que salir iguales
/// byte a byte; el `diff` es la prueba.
fn plan_cmd(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("uso: forge plan <config.json> [plan.json]");
        return ExitCode::from(2);
    }
    let cfg_txt = match fs::read_to_string(&args[0]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", args[0]);
            return ExitCode::FAILURE;
        }
    };
    let cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} no es una config válida: {e}", args[0]);
            return ExitCode::FAILURE;
        }
    };

    let (plan, avisos) = match forge_api::plan::build_download_plan(&cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("No se pudo resolver el plan: {e}");
            return ExitCode::FAILURE;
        }
    };
    for aviso in &avisos {
        eprintln!("{}", aviso.0);
    }

    // el TS escribe el plan con `JSON.stringify(plan, null, 2)` y un salto final
    let json = match serde_json::to_string_pretty(&plan) {
        Ok(j) => format!("{j}\n"),
        Err(e) => {
            eprintln!("No se pudo serializar el plan: {e}");
            return ExitCode::FAILURE;
        }
    };
    match args.get(1) {
        Some(destino) => match fs::write(destino, json) {
            Ok(()) => {
                println!("plan escrito en {destino}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("No se pudo escribir {destino}: {e}");
                ExitCode::FAILURE
            }
        },
        None => {
            print!("{json}");
            ExitCode::SUCCESS
        }
    }
}

/// `forge doctor [--dir D] [--json]`.
///
/// Lee `openblock.json` del directorio, como el `doctorCmd` del TS: el server tiene
/// que estar instalado. El `serverDir` que trae el JSON puede ser de otro SO (un zip
/// generado en Windows y abierto en Linux), así que si no existe se usa `--dir`.
fn doctor_cmd(args: &[String]) -> ExitCode {
    let mut dir = ".".to_string();
    let mut json = false;
    let mut boot = false;
    let mut yes = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json = true,
            // el TS arranca el server por defecto; aquí hay que pedirlo, porque el
            // boot test lanza un server de verdad y tarda minutos
            "--boot" => boot = true,
            // sin TTY no hay confirmación, así que `--yes` es «aplica lo seguro»,
            // exactamente igual que en el TS
            "--yes" => yes = true,
            "--dir" => {
                i += 1;
                match args.get(i) {
                    Some(d) => dir = d.clone(),
                    None => {
                        eprintln!("--dir necesita un directorio");
                        return ExitCode::from(2);
                    }
                }
            }
            otro => {
                eprintln!("opción desconocida para doctor: {otro}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }

    let raiz = resolver(&dir);
    let cfg_path = raiz.join("openblock.json");
    let cfg_txt = match fs::read_to_string(&cfg_path) {
        Ok(t) => t,
        Err(_) => {
            eprintln!(
                "No existe {}: instala el server primero",
                cfg_path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let mut cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("openblock.json inválido: {e}");
            return ExitCode::FAILURE;
        }
    };
    // el serverDir embebido puede ser de otra máquina
    if !Path::new(&cfg.server_dir).exists() {
        cfg.server_dir = raiz.to_string_lossy().to_string();
    }

    let donde = PathBuf::from(&cfg.server_dir);
    // `doctorMode` del TS: `readOnly = json || (!tty && !yes)`. Como aquí no hay
    // TTY, el doctor solo escribe si se pide con `--yes`.
    let modo = if yes && !json {
        doctor::Modo::Aplicar
    } else {
        doctor::Modo::SoloLectura
    };
    if boot {
        eprintln!("== Dry run: arrancando el server en modo test ==");
    }
    let informe = doctor::ejecutar(&mut cfg, &donde, boot, modo);

    if json {
        match serde_json::to_string_pretty(&informe) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("No se pudo serializar el informe: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let (errores, avisos, infos) = doctor::contar(&informe);
        println!("\n== Doctor: {} ({}) ==", cfg.name, cfg.engine.as_str());
        if informe.boot_tested {
            println!(
                "{}",
                if informe.boot_ok {
                    "✓ El server arranca (Done)."
                } else {
                    "✗ El server no completó el arranque."
                }
            );
        }
        for f in &informe.findings {
            println!("  [{}] {} — {}", f.severity, f.code, f.message);
        }
        println!(
            "\n{errores} errores, {avisos} avisos, {infos} informativos · {} ({})",
            informe.grade, informe.score
        );
        if !informe.pending.is_empty() {
            println!("\nPendiente de arreglar:");
            for p in &informe.pending {
                println!("  - {p}");
            }
        }
    }
    // `if (report.score < 40) process.exitCode = 1`: el corte es 40, no la nota
    // «SANO»: un server con avisos sale con 0 para que un script lo pueda usar
    if informe.score < 40 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// `forge install <config.json> [--force] [--dry-run]`.
///
/// Es el camino completo: resolver el plan, escribir el proyecto, dejar el Java en
/// `runtime/`, bajar el engine, los plugins y los mods, y ejecutar el installer de
/// Forge/NeoForge si toca, y con --pregen arranca el server y lanza Chunky por
/// RCON. La pregeneración va apagada por defecto —en el TS es al revés, con
/// --no-pregen— porque son 20 minutos de espera máxima en un install.
fn install_cmd(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("uso: forge install <config.json> [--force] [--dry-run]");
        return ExitCode::from(2);
    }
    let cfg_txt = match fs::read_to_string(&args[0]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", args[0]);
            return ExitCode::FAILURE;
        }
    };
    let mut cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} no es una config válida: {e}", args[0]);
            return ExitCode::FAILURE;
        }
    };
    let mut opts = apply::OpcionesApply {
        cores: num_cpus(),
        ..Default::default()
    };
    // la pregeneracion NO va por defecto: arranca un server y espera a Done (hasta
    // 20 minutos). En el TS es al reves, con --no-pregen para quitarla.
    let mut pregen = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--force" => opts.force = true,
            "--dry-run" => opts.dry_run = true,
            "--pregen" => pregen = true,
            "--dir" => {
                i += 1;
                match args.get(i) {
                    Some(d) => cfg.server_dir = d.clone(),
                    None => {
                        eprintln!("--dir necesita un directorio");
                        return ExitCode::from(2);
                    }
                }
            }
            "--cores" => {
                i += 1;
                match args.get(i).and_then(|c| c.parse::<u32>().ok()) {
                    Some(c) => opts.cores = c,
                    None => {
                        eprintln!("--cores necesita un numero");
                        return ExitCode::from(2);
                    }
                }
            }
            otro => {
                eprintln!("opcion desconocida para install: {otro}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }

    // la contraseña de RCON, si está habilitado y vacía, se genera aquí: el server
    // no puede arrancar con RCON y sin contraseña (`ensureRconPassword`)
    if cfg.rcon.enabled && cfg.rcon.password.is_empty() {
        cfg.rcon.password = doctor::password_aleatorio();
    }

    if let Err(e) = apply::apply_config(&mut cfg, &opts, &mut |l| println!("{l}")) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    // 4) pregeneracion opcional via RCON + Chunky
    if pregen && cfg.pregen_radius > 0 {
        let dir = resolver(&cfg.server_dir);
        let (plan, _) = match forge_api::plan::build_download_plan(&cfg) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("No se pudo resolver el plan para la pregeneracion: {e}");
                return ExitCode::FAILURE;
            }
        };
        let java_path = std::fs::read_to_string(dir.join("runtime").join("java.path"))
            .map(|t| t.trim().to_string())
            .unwrap_or_else(|_| "java".to_string());
        if let Err(e) = apply::pregenerate(&cfg, &dir, &java_path, &plan, &mut |l| println!("{l}"))
        {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

/// `forge generate <config.json> [--dir D] [--zip Z] [--cores N]`.
///
/// Resuelve el plan, genera el proyecto y lo escribe. Con `--zip` además lo
/// empaqueta para llevarlo a otra máquina. Es el `openblock generate` del TS, pero
/// dirigido por el fichero de config: la superficie de flags del modo avanzado
/// (`--engine`, `--ram-mb`, `--custom`…) todavía no está portada.
fn generate_cmd(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("uso: forge generate <config.json> [--dir D] [--zip salida.zip] [--cores N]");
        return ExitCode::from(2);
    }
    let cfg_txt = match fs::read_to_string(&args[0]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", args[0]);
            return ExitCode::FAILURE;
        }
    };
    let mut cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} no es una config válida: {e}", args[0]);
            return ExitCode::FAILURE;
        }
    };
    let mut dir: Option<String> = None;
    let mut zip_salida: Option<String> = None;
    let mut cores = num_cpus();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                dir = args.get(i).cloned();
                if dir.is_none() {
                    eprintln!("--dir necesita un directorio");
                    return ExitCode::from(2);
                }
            }
            "--zip" => {
                i += 1;
                zip_salida = args.get(i).cloned();
                if zip_salida.is_none() {
                    eprintln!("--zip necesita un fichero de salida");
                    return ExitCode::from(2);
                }
            }
            "--cores" => {
                i += 1;
                match args.get(i).and_then(|c| c.parse::<u32>().ok()) {
                    Some(c) => cores = c,
                    None => {
                        eprintln!("--cores necesita un número");
                        return ExitCode::from(2);
                    }
                }
            }
            otro => {
                eprintln!("opción desconocida para generate: {otro}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    if let Some(d) = dir {
        cfg.server_dir = d;
    }

    let (plan, avisos) = match forge_api::plan::build_download_plan(&cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("No se pudo resolver el plan: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("== Plan == {}", plan.engine_resolved);
    for e in &plan.entries {
        println!("  [{}] {}", e.kind.as_str(), e.label);
    }
    for a in &avisos {
        eprintln!("{}", a.0);
    }

    let files = forge_core::generators::generate_project(&cfg, &plan, cores);
    let destino = resolver(&cfg.server_dir);
    if let Err(e) = apply::write_project(&destino, &files) {
        eprintln!("No se pudo escribir en {}: {e}", destino.display());
        return ExitCode::FAILURE;
    }
    println!(
        "✓ Proyecto generado en {} ({} ficheros)",
        destino.display(),
        files.len()
    );

    if let Some(z) = zip_salida {
        let bytes = match apply::zip_project(&files) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("No se pudo empaquetar: {e}");
                return ExitCode::FAILURE;
            }
        };
        let ruta_zip = resolver(&z);
        if let Err(e) = fs::write(&ruta_zip, bytes) {
            eprintln!("No se pudo escribir {}: {e}", ruta_zip.display());
            return ExitCode::FAILURE;
        }
        println!("✓ Zip portable: {}", ruta_zip.display());
    }
    println!("En otra máquina: ./install.sh (Linux) o install.ps1 (Windows) y luego start.");
    ExitCode::SUCCESS
}

/// `forge self-test [--mc X] [--port N] [--keep]` — instala, arranca y diagnostica.
///
/// Es el mismo smoke test que corre el CI del TS: si sale 0, el ciclo entero
/// funciona. Instala en un directorio temporal y lo borra al terminar (`--keep` lo
/// conserva para mirarlo).
fn self_test_cmd(args: &[String]) -> ExitCode {
    let mut opts = selftest::OpcionesSelfTest {
        cores: num_cpus(),
        ..Default::default()
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--keep" => opts.keep = true,
            "--mc" => {
                i += 1;
                match args.get(i) {
                    Some(mc) => opts.mc = Some(mc.clone()),
                    None => {
                        eprintln!("--mc necesita una versión");
                        return ExitCode::from(2);
                    }
                }
            }
            "--port" => {
                i += 1;
                match args.get(i).and_then(|p| p.parse::<u16>().ok()) {
                    Some(p) => opts.port = Some(p),
                    None => {
                        eprintln!("--port necesita un número");
                        return ExitCode::from(2);
                    }
                }
            }
            otro => {
                eprintln!("opción desconocida para self-test: {otro}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    ExitCode::from(selftest::run_self_test(&opts) as u8)
}

/// `forge backup <dir>` — ejecuta el script de backup del server.
fn backup_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.first() else {
        eprintln!("uso: forge backup <dir>");
        return ExitCode::from(2);
    };
    match apply::run_backup(dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// `forge stop <dir>` — apaga el server por RCON.
///
/// El `serverDir` que trae el `openblock.json` puede ser de otra máquina, así que
/// si no existe se usa el directorio del propio fichero, como en el doctor.
fn stop_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.first() else {
        eprintln!("uso: forge stop <dir>");
        return ExitCode::from(2);
    };
    let raiz = resolver(dir);
    let cfg_path = raiz.join("openblock.json");
    let cfg_txt = match fs::read_to_string(&cfg_path) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("No existe {}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let mut cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("openblock.json inválido: {e}");
            return ExitCode::FAILURE;
        }
    };
    if !Path::new(&cfg.server_dir).exists() {
        cfg.server_dir = raiz.to_string_lossy().to_string();
    }
    match apply::stop_server(&cfg) {
        Ok(()) => {
            println!("stop enviado a 127.0.0.1:{}", cfg.rcon.port);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("No se pudo apagar el server: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `path.resolve` de Node: absoluta y sin `.`, resolviendo lo relativo contra el cwd.
pub(crate) fn resolver(p: &str) -> PathBuf {
    let ruta = Path::new(p);
    if ruta.is_absolute() {
        ruta.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(ruta)
    }
}

fn latest_cmd() -> ExitCode {
    match forge_api::mojang::latest_release() {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("No se pudo consultar Mojang: {e}");
            ExitCode::FAILURE
        }
    }
}

fn detect_cmd(args: &[String]) -> ExitCode {
    // `--raw-bytes` lo usa `tests/detect.test.ts` para comparar la detección de
    // Rust contra `os.totalmem()`/`os.release()` del TS sin depender de medidas
    // instantáneas, que cambian entre dos ejecuciones sin que nada esté mal.
    if args.iter().any(|a| a == "--raw-bytes") {
        let crudo = detect::bytes_crudos();
        match serde_json::to_string_pretty(&crudo) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("No se pudo serializar: {e}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }

    let perfil = detect::detect_hardware();
    if args.iter().any(|a| a == "--json") {
        match serde_json::to_string_pretty(&perfil) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("No se pudo serializar el perfil: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let tier = forge_core::hw::tiers::classify_hardware(&perfil);
        let ram = forge_core::hw::tiers::recommended_ram_mb(&perfil, true);
        let gc = forge_core::hw::tiers::recommended_gc(&perfil);
        println!("== Hardware ==");
        println!(
            "  Host        {} ({} {}, {})",
            perfil.hostname, perfil.os, perfil.os_version, perfil.arch
        );
        println!(
            "  CPU         {} — {} hilos",
            perfil.cpu_model, perfil.cores
        );
        println!(
            "  RAM         {} GB (libres {} GB) + swap {} GB",
            perfil.ram_total_mb / 1024,
            perfil.ram_free_mb / 1024,
            perfil.swap_total_mb / 1024
        );
        println!(
            "  Disco       {} (libres {} GB)",
            perfil.disk_kind.as_str(),
            perfil.disk_free_mb / 1024
        );
        println!(
            "  Tier        {} — hasta {} jugadores",
            tier.label, tier.max_players
        );
        println!(
            "  Recomendado {}, {}, vd {}/{}, -Xmx ~{ram} MB",
            tier.recommended_engine,
            gc.to_uppercase(),
            tier.view_distance,
            tier.simulation_distance
        );
    }
    ExitCode::SUCCESS
}

fn dump_cmd(args: &[String]) -> ExitCode {
    if args.len() < 3 {
        return uso();
    }
    let cfg_path = Path::new(&args[0]);
    let plan_path = Path::new(&args[1]);
    let out_dir = Path::new(&args[2]);
    let cores: u32 = args
        .get(3)
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(num_cpus);

    let cfg_txt = match fs::read_to_string(cfg_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config inválida en {}: {e}", cfg_path.display());
            return ExitCode::FAILURE;
        }
    };
    let plan_txt = match fs::read_to_string(plan_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("No se pudo leer {}: {e}", plan_path.display());
            return ExitCode::FAILURE;
        }
    };
    let plan: forge_core::plan::DownloadPlan = match serde_json::from_str(&plan_txt) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Plan inválido en {}: {e}", plan_path.display());
            return ExitCode::FAILURE;
        }
    };

    let errores = forge_core::generators::validate(&cfg);
    if !errores.is_empty() {
        eprintln!("Config inválida:");
        for e in &errores {
            eprintln!("  - {e}");
        }
        return ExitCode::FAILURE;
    }

    let files = forge_core::generators::generate_project(&cfg, &plan, cores);
    let mut escritos = 0usize;
    for (nombre, contenido) in &files {
        let destino = out_dir.join(nombre);
        if let Some(padre) = destino.parent() {
            if let Err(e) = fs::create_dir_all(padre) {
                eprintln!("No se pudo crear {}: {e}", padre.display());
                return ExitCode::FAILURE;
            }
        }
        // LF en todo; CRLF solo en .bat, igual que `writeProject` del TS
        let eol = if nombre.ends_with(".bat") {
            "\r\n"
        } else {
            "\n"
        };
        let normalizado = contenido.replace("\r\n", "\n").replace('\n', eol);
        if let Err(e) = fs::write(&destino, normalizado) {
            eprintln!("No se pudo escribir {}: {e}", destino.display());
            return ExitCode::FAILURE;
        }
        escritos += 1;
    }

    println!(
        "Proyecto {} volcado en {} ({escritos} ficheros)",
        cfg.name,
        out_dir.display()
    );
    ExitCode::SUCCESS
}

/// Cores lógicos sin traer una dependencia: `available_parallelism` es de std.
fn num_cpus() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
}
