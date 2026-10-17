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

mod advanced;
mod apply;
mod bench;
mod detect;
mod doctor;
mod guided;
mod java;
mod rcon;
mod selftest;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// El texto de ayuda, portado de `src/help.ts`.
///
/// El del TS dice `openblock` y lista los comandos del TS; este dice `forge` y lista
/// los que existen aquí, porque una ayuda que ofrece comandos que no están es peor
/// que no tenerla. Los que faltan respecto al TS van marcados al final.
const HELP: &str = r#"
forge — forja servidores de Minecraft según tu hardware (Windows/Linux)

  forge bench                     Mide CPU y disco (~2 s) y saca el perfil
  forge detect [--json]           Hardware + tier + recomendación
  forge plan <config> [out.json]  Resuelve el plan contra las APIs
  forge generate [config] [...]   Genera el proyecto; --zip out.zip lo empaqueta
  forge install [config] [...]    Como generate, y además baja todo y deja el
                                  server listo (Java, engine, plugins, installer)
  forge dump <config> <plan> <dir> [n]
                                  Escribe el proyecto desde config y plan
  forge info <dir>                Config del server instalado
  forge doctor [--dir D]          Mini-IA: dry run del server, errores y avisos
      --boot                      arranca el server de verdad y lee su log
      --yes                       aplica los fixes seguros (los considerables no)
      --json                      informe JSON (para scripts). Implica solo lectura
  forge backup <dir>              Ejecuta backup.sh/.ps1 ahora
  forge stop <dir>                Apaga por RCON (tiene que estar arriba)
  forge self-test                 Instala un Paper de verdad, lo arranca y lo
                                  diagnostica; falla si algo no cuadra. Para CI
      --mc 1.20.4 --port 25599 --keep
  forge latest                    Última release de Minecraft (consulta Mojang)
  forge rcon <host> <puerto> <cmd>
                                  Manda un comando por RCON
                                  (pass: OPENBLOCK_RCON_PASSWORD)
  forge help                      Esta ayuda

Flags de config (generate/install), con la precedencia flag > --config > hardware:
  --config openblock.json         base reimportable (round-trip)
  --name X --mc 1.21.4 --engine paper|purpur|folia|fabric|quilt|forge|neoforge|vanilla
  --ram-mb 8192 --players 20 --port 25565 --vd 8 --sd 6 --dir ./server
  --profile conservative|balanced|aggressive --pregen 4000
  --modpack <url-mrpack> --target linux|windows|both
  --no-service --no-backup --no-rcon --no-pregen --dry-run --yes --force
  --build N (paper/purpur/folia/forge/neoforge), --loader-build N,
  --installer-build N (fabric/quilt), --custom url1,ruta2 (plugins/mods propios)
  --cores N (hilos para los scripts)

Falta respecto al CLI de TypeScript: el modo guiado (`guided`) y `advanced`
—aquí las flags del modo avanzado se dan a `generate` y a `install`—.

Project format: openblock.json round-trip, como DistroForge.
"#;

fn uso() -> ExitCode {
    eprintln!("uso: forge <comando> [opciones]");
    eprintln!();
    eprintln!("  detect [--json]                 hardware detectado (JSON igual que el TS)");
    eprintln!("  bench                           mide CPU y disco (~4 s) y saca el perfil");
    eprintln!("  info <dir>                      config del server instalado");
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
        Some("bench") => bench_cmd(),
        Some("info") => info_cmd(&args[1..]),
        Some("guided") => match guided::guided() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Some("help") | Some("--help") | Some("-h") => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
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
    let a = match advanced::parse_avanzados(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let mut cfg = match advanced::construir_config(&a) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let opts = apply::OpcionesApply {
        cores: a.cores.unwrap_or_else(num_cpus),
        dry_run: a.dry_run,
        force: a.force,
    };

    if let Err(e) = apply::apply_config(&mut cfg, &opts, &mut |l| println!("{l}")) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    // 4) pregeneracion opcional via RCON + Chunky (no va por defecto: arranca un
    // server y espera a `Done`, hasta 20 minutos)
    if a.pregen.unwrap_or(0) > 0 && !a.no_pregen {
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
/// `forge bench` — mide CPU y disco (~4 s) y saca el perfil con la medida dentro.
///
/// Es el `openblock bench` del TS. Imprime JSON, igual que `forge detect`: es lo que
/// un script consume, y la tabla con colores del TS no se puede comparar.
fn bench_cmd() -> ExitCode {
    eprintln!("Midiendo CPU y disco...");
    let mut hw = detect::detect_hardware();
    hw.bench = Some(bench::run_benchmark());
    match serde_json::to_string_pretty(&hw) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("No se pudo serializar: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `forge info <dir>` — la config del server instalado, como una tabla.
fn info_cmd(args: &[String]) -> ExitCode {
    let dir = args.first().map(String::as_str).unwrap_or(".");
    let raiz = resolver(dir);
    let p = raiz.join("openblock.json");
    let cfg_txt = match fs::read_to_string(&p) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("No existe {}: instala el server primero", p.display());
            return ExitCode::FAILURE;
        }
    };
    let cfg: forge_core::ServerConfig = match serde_json::from_str(&cfg_txt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("openblock.json inválido: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("\n== {} ==", cfg.name);
    for (k, v) in [
        (
            "Engine",
            format!("{} {}", cfg.engine.as_str(), cfg.mc_version),
        ),
        ("RAM", format!("{} MB", cfg.ram_mb)),
        ("Puerto", cfg.port.to_string()),
        ("Jugadores", cfg.players.to_string()),
        ("Perfil", cfg.performance.as_str().to_string()),
        ("Pregen", cfg.pregen_radius.to_string()),
        ("Directorio", cfg.server_dir.clone()),
        (
            "Servicio",
            if cfg.service.enabled { "sí" } else { "no" }.to_string(),
        ),
    ] {
        println!("  {k:<12} {v}");
    }
    ExitCode::SUCCESS
}

fn generate_cmd(args: &[String]) -> ExitCode {
    let a = match advanced::parse_avanzados(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if a.help {
        return uso();
    }
    let cfg = match advanced::construir_config(&a) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // `--zip` y `--cores` los lee el parser de flags del modo avanzado, que los
    // conoce; aquí solo se usan
    let zip_salida = a.zip.clone();
    let cores = a.cores.unwrap_or_else(num_cpus);

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
    for aviso in &avisos {
        eprintln!("{}", aviso.0);
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
