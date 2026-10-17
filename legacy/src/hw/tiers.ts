import type { HardwareProfile, Tier, TierId } from "../types.js";

/**
 * Clasificación del hardware en tiers. Sin benchmark usa solo datos duros;
 * con benchmark ajusta el score de CPU (opción nuclear: `openblock bench`).
 */
export const TIERS: Record<TierId, Tier> = {
  potato: {
    id: "potato",
    label: "Patata",
    maxPlayers: 5,
    viewDistance: 4,
    simulationDistance: 4,
    pregenRadius: 0,
    recommendedEngine: "paper",
    gc: "g1",
    dedicatedRamPct: 0.6,
  },
  low: {
    id: "low",
    label: "Bajo",
    maxPlayers: 10,
    viewDistance: 6,
    simulationDistance: 5,
    pregenRadius: 2000,
    recommendedEngine: "paper",
    gc: "g1",
    dedicatedRamPct: 0.65,
  },
  mid: {
    id: "mid",
    label: "Medio",
    maxPlayers: 20,
    viewDistance: 8,
    simulationDistance: 6,
    pregenRadius: 4000,
    recommendedEngine: "purpur",
    gc: "g1",
    dedicatedRamPct: 0.7,
  },
  high: {
    id: "high",
    label: "Alto",
    maxPlayers: 40,
    viewDistance: 10,
    simulationDistance: 8,
    pregenRadius: 6000,
    recommendedEngine: "purpur",
    gc: "g1",
    dedicatedRamPct: 0.75,
  },
  enthusiast: {
    id: "enthusiast",
    label: "Entusiasta",
    maxPlayers: 60,
    viewDistance: 12,
    simulationDistance: 10,
    pregenRadius: 10000,
    recommendedEngine: "folia",
    gc: "zgc",
    dedicatedRamPct: 0.8,
  },
};

export function classifyHardware(hw: HardwareProfile): Tier {
  let score = 0;
  // RAM: hasta 16GB puntúan, más aporte marginal
  score += Math.min(40, Math.round((hw.ramTotalMB / 1024) * 2.5));
  // cores lógicos: hasta 16
  score += Math.min(30, hw.cores * 2);
  // disco: NVMe 20, SSD 14, HDD 4
  score += hw.diskKind === "nvme" ? 20 : hw.diskKind === "ssd" ? 14 : hw.diskKind === "hdd" ? 4 : 12;
  // virtualizado: penalización leve (paginación de host, IO compartido)
  if (hw.virtualized) score -= 5;
  // benchmark: normaliza el peso de CPU (10 pts máx) contra una referencia media moderna
  if (hw.bench && hw.bench.cpuScore > 0) {
    // referencia: ~7k ops/core/ms ≈ desktop moderno multithread (BigInt xorshift)
    score += Math.min(10, Math.round((hw.bench.cpuScore / 7000) * 10));
  }

  if (hw.ramTotalMB < 4096 || hw.cores < 2) return TIERS.potato;
  if (score >= 85) return TIERS.enthusiast;
  if (score >= 65) return TIERS.high;
  if (score >= 45) return TIERS.mid;
  if (score >= 30) return TIERS.low;
  return TIERS.potato;
}

/** -Xmx recomendado según RAM total y si la máquina está dedicada al server */
export function recommendedRamMB(hw: HardwareProfile, dedicated: boolean): number {
  const pct = dedicated ? TIERS[classifyHardware(hw).id].dedicatedRamPct : 0.45;
  const raw = Math.round((hw.ramTotalMB * pct) / 512) * 512;
  // mínimo jugable 1024, y dejar 1.5GB de aire al sistema SIEMPRE
  const cap = Math.max(1024, hw.ramTotalMB - 1536);
  return Math.min(raw, cap);
}

/** GC recomendado: ZGC generational solo con RAM de sobra y bastantes cores */
export function recommendedGc(hw: HardwareProfile): "g1" | "zgc" {
  return hw.ramTotalMB >= 16 * 1024 && hw.cores >= 8 ? "zgc" : "g1";
}
