//! Paridad byte a byte con el núcleo TypeScript.
//!
//! Los ficheros de `tests/golden/` los genera el TS (`npm run parity:golden`) a
//! partir de `tests/fixtures/parity-config.json`. Aquí el núcleo Rust genera lo
//! mismo desde el mismo fixture y se comparan: si alguien toca un generador en un
//! lado y no en el otro, esto lo dice con el fichero exacto.
//!
//! Es el `diff -r` que DistroForge documenta como procedimiento manual, pero
//! dentro de la suite de tests: la paridad deja de depender de que alguien se
//! acuerde de comparar a mano.

use forge_core::generators::{jvm, ops, paper, properties, service};
use forge_core::types::ServerConfig;
use std::fs;
use std::path::{Path, PathBuf};

/// Raíz del repo (dos niveles por encima de `rust/crates/forge-core`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn fixture() -> ServerConfig {
    let p = repo_root().join("tests/fixtures/parity-config.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).unwrap_or_else(|e| panic!("fixture inválido: {e}"))
}

fn golden(nombre: &str) -> String {
    let p = repo_root()
        .join("rust/crates/forge-core/tests/golden/leaves")
        .join(nombre);
    fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no se pudo leer el golden {}: {e}", p.display()))
}

/// Posición 0-based del primer byte distinto, para señalar dónde se separan.
fn primera_diferencia(a: &str, b: &str) -> Option<usize> {
    a.bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .or_else(|| {
            if a.len() == b.len() {
                None
            } else {
                Some(a.len().min(b.len()))
            }
        })
}

/// Compara un generador con su golden; si difieren, enseña la línea exacta.
fn assert_paridad(nombre: &str, esperado: &str, obtenido: &str) {
    if esperado == obtenido {
        return;
    }
    let pos = primera_diferencia(esperado, obtenido).unwrap_or(0);
    let linea = esperado[..pos].lines().count().max(1);
    let esperado_linea = esperado.lines().nth(linea - 1).unwrap_or("");
    let obtenido_linea = obtenido.lines().nth(linea - 1).unwrap_or("");
    panic!(
        "paridad rota en «{nombre}» (línea {linea}, byte {pos})\n  TS     ({linea}): {esperado_linea:?}\n  Rust   ({linea}): {obtenido_linea:?}\n  longitudes: TS {} / Rust {}",
        esperado.len(),
        obtenido.len()
    );
}

#[test]
fn paridad_generadores_de_hoja() {
    let cfg = fixture();
    let cores = 16;
    let gc = forge_core::generators::gc_for(cfg.ram_mb, cores);
    let user = cfg
        .service
        .user
        .clone()
        .unwrap_or_else(|| "minecraft".to_string());

    let casos: Vec<(&str, String)> = vec![
        ("jvm-args.txt", jvm::jvm_args_line(&cfg, gc, cores)),
        ("server.properties", properties::server_properties(&cfg)),
        ("paper-global.yml", paper::paper_global_yml()),
        (
            "paper-world-defaults.yml",
            paper::paper_world_defaults_yml(&cfg),
        ),
        ("spigot.yml", paper::spigot_yml(&cfg)),
        ("bukkit.yml", paper::bukkit_yml(&cfg)),
        ("firewall.sh", ops::firewall_sh(&cfg)),
        ("firewall.ps1", ops::firewall_ps1(&cfg)),
        ("backup.sh", ops::backup_sh(&cfg)),
        ("backup.ps1", ops::backup_ps1(&cfg)),
        ("openblock-backup.timer", ops::backup_timer(&cfg)),
        ("openblock.service", service::systemd_unit(&cfg, &user)),
        (
            "install-service.sh",
            service::install_service_sh(&cfg, &user),
        ),
        ("winsw.xml", service::winsw_config(&cfg)),
        ("install-service.ps1", service::install_service_ps1(&cfg)),
        ("remove-service.ps1", service::remove_service_ps1(&cfg)),
    ];

    let mut fallos = Vec::new();
    for (nombre, obtenido) in &casos {
        let esperado = golden(nombre);
        if esperado != *obtenido {
            fallos.push(*nombre);
            // se compara aquí también para que el pánico diga la línea exacta
            assert_paridad(nombre, &esperado, obtenido);
        }
    }
    assert!(fallos.is_empty(), "generadores sin paridad: {fallos:?}");
}

/// Todo generador de hoja tiene que estar en el golden: si se añade uno al TS y
/// no aquí, el test lo dice en vez de pasar por omisión.
#[test]
fn el_golden_no_tiene_generadores_sin_cubrir() {
    let dir = repo_root().join("rust/crates/forge-core/tests/golden/leaves");
    let mut nombres: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("no existe {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    nombres.sort();

    const CUBIERTOS: [&str; 16] = [
        "backup.ps1",
        "backup.sh",
        "bukkit.yml",
        "firewall.ps1",
        "firewall.sh",
        "install-service.ps1",
        "install-service.sh",
        "jvm-args.txt",
        "openblock-backup.timer",
        "openblock.service",
        "paper-global.yml",
        "paper-world-defaults.yml",
        "remove-service.ps1",
        "server.properties",
        "spigot.yml",
        "winsw.xml",
    ];
    let esperados: Vec<String> = CUBIERTOS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        nombres, esperados,
        "el golden y los generadores cubiertos no coinciden"
    );
}
