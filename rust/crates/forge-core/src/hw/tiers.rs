//! Clasificación del hardware en tiers, portada de `src/hw/tiers.ts`.
//!
//! Es puro: recibe un `HardwareProfile` ya detectado y solo calcula. La detección
//! —leer `/proc`, ejecutar PowerShell— sí toca el sistema y vive en el CLI, así
//! que este módulo entra en el núcleo y tiene paridad byte a byte como el resto.

use crate::types::{DiskKind, EngineId, HardwareProfile, Tier, TierId};

/// Los cinco tiers (`TIERS`).
pub const TIERS: &[Tier] = &[
    Tier {
        id: TierId::Potato,
        label: "Patata",
        max_players: 5,
        view_distance: 4,
        simulation_distance: 4,
        pregen_radius: 0,
        recommended_engine: EngineId::Paper,
        gc: "g1",
        dedicated_ram_pct: 0.6,
    },
    Tier {
        id: TierId::Low,
        label: "Bajo",
        max_players: 10,
        view_distance: 6,
        simulation_distance: 5,
        pregen_radius: 2000,
        recommended_engine: EngineId::Paper,
        gc: "g1",
        dedicated_ram_pct: 0.65,
    },
    Tier {
        id: TierId::Mid,
        label: "Medio",
        max_players: 20,
        view_distance: 8,
        simulation_distance: 6,
        pregen_radius: 4000,
        recommended_engine: EngineId::Purpur,
        gc: "g1",
        dedicated_ram_pct: 0.7,
    },
    Tier {
        id: TierId::High,
        label: "Alto",
        max_players: 40,
        view_distance: 10,
        simulation_distance: 8,
        pregen_radius: 6000,
        recommended_engine: EngineId::Purpur,
        gc: "g1",
        dedicated_ram_pct: 0.75,
    },
    Tier {
        id: TierId::Enthusiast,
        label: "Entusiasta",
        max_players: 60,
        view_distance: 12,
        simulation_distance: 10,
        pregen_radius: 10000,
        recommended_engine: EngineId::Folia,
        gc: "zgc",
        dedicated_ram_pct: 0.8,
    },
];

/// Un tier por id. Total, porque `TierId` es un enum cerrado.
pub fn tier_by_id(id: TierId) -> &'static Tier {
    TIERS
        .iter()
        .find(|t| t.id == id)
        .expect("todo TierId tiene su Tier en TIERS")
}

/// Clasifica el hardware en un tier (`classifyHardware`).
///
/// Sin benchmark usa solo datos duros; con benchmark ajusta el score de CPU
/// (opción nuclear: `openblock bench`).
pub fn classify_hardware(hw: &HardwareProfile) -> &'static Tier {
    let mut score: u64 = 0;
    // RAM: hasta 16GB puntúan, más aporte marginal
    score += std::cmp::min(40, ((hw.ram_total_mb as f64 / 1024.0) * 2.5).round() as u64);
    // cores lógicos: hasta 16
    score += std::cmp::min(30, u64::from(hw.cores) * 2);
    // disco: NVMe 20, SSD 14, HDD 4, desconocido 12
    score += match hw.disk_kind {
        DiskKind::Nvme => 20,
        DiskKind::Ssd => 14,
        DiskKind::Hdd => 4,
        DiskKind::Unknown => 12,
    };
    // virtualizado: penalización leve (paginación de host, IO compartido)
    if hw.virtualized {
        score = score.saturating_sub(5);
    }
    // benchmark: normaliza el peso de CPU (10 pts máx) contra una referencia media
    if let Some(bench) = &hw.bench {
        if bench.cpu_score > 0.0 {
            // referencia: ~7k ops/core/ms ≈ desktop moderno multithread
            score += std::cmp::min(10, ((bench.cpu_score / 7000.0) * 10.0).round() as u64);
        }
    }

    if hw.ram_total_mb < 4096 || hw.cores < 2 {
        return tier_by_id(TierId::Potato);
    }
    if score >= 85 {
        return tier_by_id(TierId::Enthusiast);
    }
    if score >= 65 {
        return tier_by_id(TierId::High);
    }
    if score >= 45 {
        return tier_by_id(TierId::Mid);
    }
    if score >= 30 {
        return tier_by_id(TierId::Low);
    }
    tier_by_id(TierId::Potato)
}

/// `-Xmx` recomendado según RAM total y si la máquina está dedicada (`recommendedRamMB`).
pub fn recommended_ram_mb(hw: &HardwareProfile, dedicated: bool) -> u64 {
    let pct = if dedicated {
        classify_hardware(hw).dedicated_ram_pct
    } else {
        0.45
    };
    let raw = ((hw.ram_total_mb as f64 * pct) / 512.0).round() as u64 * 512;
    // mínimo jugable 1024, y dejar 1.5GB de aire al sistema SIEMPRE
    let cap = std::cmp::max(1024, hw.ram_total_mb.saturating_sub(1536));
    std::cmp::min(raw, cap)
}

/// GC recomendado: ZGC generacional solo con RAM de sobra y bastantes cores
/// (`recommendedGc`).
pub fn recommended_gc(hw: &HardwareProfile) -> &'static str {
    if hw.ram_total_mb >= 16 * 1024 && hw.cores >= 8 {
        "zgc"
    } else {
        "g1"
    }
}
