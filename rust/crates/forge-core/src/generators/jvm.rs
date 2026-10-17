//! Flags de JVM, portados de `src/generators/jvm.ts`.
//!
//! G1GC sintonizado con los flags de Aikar (fórmulas exactas de
//! <https://mcflags.emc.gs>) o ZGC generacional para heaps grandes.

use crate::types::ServerConfig;

/// Reparto de hilos del GC (`gcThreadCounts`).
///
/// `(cores - 8) * 5 / 8` con división entera es el `Math.floor` del TS: los dos
/// truncan hacia cero y aquí `cores` nunca es negativo.
pub fn gc_thread_counts(cores: u32) -> (u32, u32) {
    let parallel = if cores < 8 {
        cores
    } else {
        8 + ((cores - 8) * 5) / 8
    };
    let conc = std::cmp::max(1, (f64::from(parallel) * 0.75).round() as u32);
    (parallel, conc)
}

/// Flags de Aikar para un heap dado (`aikarFlags`).
pub fn aikar_flags(ram_mb: u32, cores: u32) -> Vec<String> {
    let heap = format!("{ram_mb}M");
    let large = ram_mb > 12 * 1024;
    let (parallel, conc) = gc_thread_counts(cores);
    vec![
        format!("-Xms{heap}"),
        format!("-Xmx{heap}"),
        "-XX:+UseG1GC".into(),
        "-XX:+ParallelRefProcEnabled".into(),
        "-XX:MaxGCPauseMillis=200".into(),
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:+DisableExplicitGC".into(),
        "-XX:+AlwaysPreTouch".into(),
        format!("-XX:ParallelGCThreads={parallel}"),
        format!("-XX:ConcGCThreads={conc}"),
        format!("-XX:G1NewSizePercent={}", if large { 40 } else { 30 }),
        format!("-XX:G1MaxNewSizePercent={}", if large { 50 } else { 40 }),
        format!("-XX:G1HeapRegionSize={}", if large { "16M" } else { "8M" }),
        "-XX:G1ReservePercent=20".into(),
        "-XX:G1HeapWastePercent=5".into(),
        "-XX:G1MixedGCCountTarget=4".into(),
        "-XX:InitiatingHeapOccupancyPercent=15".into(),
        "-XX:G1MixedGCLiveThresholdPercent=90".into(),
        "-XX:G1RSetUpdatingPauseTimePercent=5".into(),
        "-XX:SurvivorRatio=32".into(),
        "-XX:+PerfDisableSharedMem".into(),
        format!("-XX:MaxTenuringThreshold={}", if large { 15 } else { 1 }),
        "-Dusing.aikars.flags=https://mcflags.emc.gs".into(),
        "-Daikars.new.flags=true".into(),
    ]
}

/// Flags de ZGC generacional (`zgcFlags`).
///
/// El heap blando se redondea a múltiplos de 512 MB con `Math.round`: en Rust el
/// `round()` de `f64` redondea **half away from zero**, igual que el de JS.
pub fn zgc_flags(ram_mb: u32, cores: u32) -> Vec<String> {
    let heap = format!("{ram_mb}M");
    let soft = format!(
        "{}M",
        ((f64::from(ram_mb) * 0.75) / 512.0).round() as u64 * 512
    );
    let (parallel, _) = gc_thread_counts(cores);
    vec![
        format!("-Xms{heap}"),
        format!("-Xmx{heap}"),
        "-XX:+UseZGC".into(),
        "-XX:+ZGenerational".into(),
        "-XX:+DisableExplicitGC".into(),
        "-XX:+AlwaysPreTouch".into(),
        format!("-XX:SoftMaxHeapSize={soft}"),
        format!("-XX:ParallelGCThreads={parallel}"),
        "-Dusing.aikars.flags=https://mcflags.emc.gs".into(),
    ]
}

/// Flags completos del server según config (`jvmFlags`).
pub fn jvm_flags(cfg: &ServerConfig, gc: &str, cores: u32) -> Vec<String> {
    let n = std::cmp::max(1, cores);
    let mut base = if gc == "zgc" {
        zgc_flags(cfg.ram_mb, n)
    } else {
        aikar_flags(cfg.ram_mb, n)
    };
    if cfg.engine.is_modded() {
        // los modpacks grandes cargan muchísimo bytecode
        base.push("-XX:MaxMetaspaceSize=1G".into());
    }
    base
}

/// Una línea para `jvm-args.txt` y los scripts de arranque (`jvmArgsLine`).
pub fn jvm_args_line(cfg: &ServerConfig, gc: &str, cores: u32) -> String {
    jvm_flags(cfg, gc, cores).join(" ")
}
