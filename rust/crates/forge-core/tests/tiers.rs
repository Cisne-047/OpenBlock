//! Tiers: mismos casos que `tests/hw.test.ts`, más paridad contra el golden del TS.

use forge_core::hw::tiers;
use forge_core::types::{BenchmarkResult, DiskKind, HardwareProfile, TierId};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

/// El `hw()` de `tests/hw.test.ts:6-20`.
fn hw(over: impl FnOnce(&mut HardwareProfile)) -> HardwareProfile {
    let mut p = HardwareProfile {
        os: "linux".into(),
        os_version: "6.8".into(),
        arch: "x64".into(),
        hostname: "test".into(),
        cpu_model: "Test CPU".into(),
        cores: 8,
        ram_total_mb: 16384,
        ram_free_mb: 8192,
        swap_total_mb: 0,
        disk_kind: DiskKind::Nvme,
        disk_free_mb: 50000,
        virtualized: false,
        bench: None,
    };
    over(&mut p);
    p
}

// ---------- classifyHardware (tests/hw.test.ts:22-42) ----------

#[test]
fn potato_poca_ram_o_un_core() {
    assert_eq!(
        tiers::classify_hardware(&hw(|p| p.ram_total_mb = 2048)).id,
        TierId::Potato
    );
    assert_eq!(
        tiers::classify_hardware(&hw(|p| p.cores = 1)).id,
        TierId::Potato
    );
}

#[test]
fn enthusiast_muchos_cores_nvme_y_ram() {
    let t = tiers::classify_hardware(&hw(|p| {
        p.cores = 24;
        p.ram_total_mb = 65536;
        p.disk_kind = DiskKind::Nvme;
    }));
    assert_eq!(t.id, TierId::Enthusiast);
}

#[test]
fn hdd_penaliza_al_menos_un_tier() {
    let t = tiers::classify_hardware(&hw(|p| {
        p.disk_kind = DiskKind::Hdd;
        p.ram_total_mb = 32768;
        p.cores = 12;
    }));
    assert_ne!(t.id, TierId::Enthusiast);
}

#[test]
fn la_vm_penaliza() {
    // el orden del TS es de mejor a peor, así que un índice mayor = peor tier
    let orden = [
        TierId::Enthusiast,
        TierId::High,
        TierId::Mid,
        TierId::Low,
        TierId::Potato,
    ];
    let indice = |id: TierId| orden.iter().position(|x| *x == id).unwrap();
    let bare = indice(tiers::classify_hardware(&hw(|p| p.virtualized = false)).id);
    let vm = indice(tiers::classify_hardware(&hw(|p| p.virtualized = true)).id);
    assert!(
        vm >= bare,
        "la VM no puede salir mejor clasificada que la máquina real"
    );
}

#[test]
fn el_benchmark_sube_el_score() {
    // con un cpuScore alto, un equipo que sin bench queda en mid puede subir
    let sin = tiers::classify_hardware(&hw(|p| {
        p.cores = 8;
        p.ram_total_mb = 16384;
        p.disk_kind = DiskKind::Hdd;
    }));
    let con = tiers::classify_hardware(&hw(|p| {
        p.cores = 8;
        p.ram_total_mb = 16384;
        p.disk_kind = DiskKind::Hdd;
        p.bench = Some(BenchmarkResult {
            cpu_score: 9000.0,
            disk_m_bs: 500.0,
            duration_ms: 1000.0,
        });
    }));
    let orden = [
        TierId::Enthusiast,
        TierId::High,
        TierId::Mid,
        TierId::Low,
        TierId::Potato,
    ];
    let indice = |id: TierId| orden.iter().position(|x| *x == id).unwrap();
    assert!(
        indice(con.id) <= indice(sin.id),
        "el bench no puede empeorar el tier"
    );
}

// ---------- recommendedRamMB (tests/hw.test.ts:44-56) ----------

#[test]
fn dedicada_asigna_mas_que_compartida() {
    let p = hw(|p| p.ram_total_mb = 32768);
    assert!(tiers::recommended_ram_mb(&p, true) > tiers::recommended_ram_mb(&p, false));
}

#[test]
fn nunca_deja_el_sistema_sin_aire() {
    // 4GB: 60% = 2400MB pero el cap (4096-1536) manda
    let p4 = hw(|p| p.ram_total_mb = 4096);
    assert!(tiers::recommended_ram_mb(&p4, true) <= 4096 - 1536);
    let p2 = hw(|p| p.ram_total_mb = 2048);
    assert!(tiers::recommended_ram_mb(&p2, true) >= 1024);
}

#[test]
fn recommended_gc_exige_ram_y_cores() {
    assert_eq!(
        tiers::recommended_gc(&hw(|p| p.ram_total_mb = 16384)),
        "zgc"
    );
    assert_eq!(tiers::recommended_gc(&hw(|p| p.ram_total_mb = 8192)), "g1");
    assert_eq!(tiers::recommended_gc(&hw(|p| p.cores = 4)), "g1");
}

// ---------- paridad de la tabla y la clasificación ----------

/// El golden lo genera el TS con varias configuraciones de hardware. Aquí se
/// comparan los cinco tiers campo a campo y la clasificación resultante, para no
/// depender de que mis casos de test cubran los mismos bordes que los suyos.
#[test]
fn tiers_y_clasificacion_coinciden_con_el_ts() {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/tiers.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    let g: serde_json::Value = serde_json::from_str(&txt).expect("golden inválido");

    // --- los cinco tiers ---
    let lista = g["tiers"].as_array().expect("tiers");
    assert_eq!(lista.len(), tiers::TIERS.len(), "número de tiers");
    for (t, gt) in tiers::TIERS.iter().zip(lista) {
        let id = gt["id"].as_str().unwrap();
        assert_eq!(t.id.as_str(), id, "id");
        assert_eq!(t.label, gt["label"].as_str().unwrap(), "label de {id}");
        assert_eq!(
            t.max_players,
            gt["maxPlayers"].as_u64().unwrap() as u32,
            "maxPlayers de {id}"
        );
        assert_eq!(
            t.view_distance,
            gt["viewDistance"].as_u64().unwrap() as u32,
            "viewDistance de {id}"
        );
        assert_eq!(
            t.simulation_distance,
            gt["simulationDistance"].as_u64().unwrap() as u32,
            "simulationDistance de {id}"
        );
        assert_eq!(
            t.pregen_radius,
            gt["pregenRadius"].as_u64().unwrap() as u32,
            "pregenRadius de {id}"
        );
        assert_eq!(
            t.recommended_engine.as_str(),
            gt["recommendedEngine"].as_str().unwrap(),
            "recommendedEngine de {id}"
        );
        assert_eq!(t.gc, gt["gc"].as_str().unwrap(), "gc de {id}");
        assert_eq!(
            t.dedicated_ram_pct,
            gt["dedicatedRamPct"].as_f64().unwrap(),
            "dedicatedRamPct de {id}"
        );
    }

    // --- clasificación de cada perfil del golden ---
    for caso in g["casos"].as_array().expect("casos") {
        let nombre = caso["nombre"].as_str().unwrap();
        let h = &caso["hw"];
        let perfil = HardwareProfile {
            os: h["os"].as_str().unwrap().to_string(),
            os_version: h["osVersion"].as_str().unwrap().to_string(),
            arch: h["arch"].as_str().unwrap().to_string(),
            hostname: h["hostname"].as_str().unwrap().to_string(),
            cpu_model: h["cpuModel"].as_str().unwrap().to_string(),
            cores: h["cores"].as_u64().unwrap() as u32,
            ram_total_mb: h["ramTotalMB"].as_u64().unwrap(),
            ram_free_mb: h["ramFreeMB"].as_u64().unwrap(),
            swap_total_mb: h["swapTotalMB"].as_u64().unwrap(),
            disk_kind: match h["diskKind"].as_str().unwrap() {
                "nvme" => DiskKind::Nvme,
                "ssd" => DiskKind::Ssd,
                "hdd" => DiskKind::Hdd,
                _ => DiskKind::Unknown,
            },
            disk_free_mb: h["diskFreeMB"].as_u64().unwrap(),
            virtualized: h["virtualized"].as_bool().unwrap(),
            bench: h.get("bench").and_then(|b| {
                b.as_object().map(|o| BenchmarkResult {
                    cpu_score: o["cpuScore"].as_f64().unwrap(),
                    disk_m_bs: o["diskMBs"].as_f64().unwrap(),
                    duration_ms: o["durationMs"].as_f64().unwrap(),
                })
            }),
        };
        assert_eq!(
            tiers::classify_hardware(&perfil).id.as_str(),
            caso["tier"].as_str().unwrap(),
            "tier de «{nombre}»"
        );
        assert_eq!(
            tiers::recommended_ram_mb(&perfil, true),
            caso["ramDedicada"].as_u64().unwrap(),
            "RAM dedicada de «{nombre}»"
        );
        assert_eq!(
            tiers::recommended_ram_mb(&perfil, false),
            caso["ramCompartida"].as_u64().unwrap(),
            "RAM compartida de «{nombre}»"
        );
        assert_eq!(
            tiers::recommended_gc(&perfil),
            caso["gc"].as_str().unwrap(),
            "gc de «{nombre}»"
        );
    }
}
