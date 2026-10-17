//! El benchmark de CPU y disco, portado de `src/hw/bench.ts`.
//!
//! La parte pura —el score, el xorshift de precisión arbitraria y la conversión a
//! MB/s— vive en el núcleo y tiene paridad byte a byte. Aquí está la **medida**: un
//! hilo por core martilleando el xorshift durante 1,8 s y una escritura secuencial
//! de 128 MB para clasificar el disco.
//!
//! La medida en sí no se puede comparar con la del TS: depende de la máquina y del
//! momento. Lo que sí se compara es todo lo que se calcula a partir de las medidas.

use forge_core::hw::bench::{cpu_score_from_ops, disk_mbs_from_secs};
use forge_core::types::BenchmarkResult;
use std::io::Write;
use std::time::{Duration, Instant};

/// Lo que dura el bucle de CPU en cada hilo (`deadline = Date.now() + 1800`).
const CPU_MS: u64 = 1800;
/// Trozo y total de la prueba de disco (`CHUNK` y `TOTAL`).
const CHUNK: usize = 4 * 1024 * 1024;
const TOTAL: usize = 128 * 1024 * 1024;

/// Mide CPU y disco (`runBenchmark`).
///
/// Las dos pruebas van **a la vez** (`Promise.all` en el TS): el disco espera al
/// `fsync` y ese tiempo se solapa con la CPU.
pub fn run_benchmark() -> BenchmarkResult {
    let t0 = Instant::now();
    let cpu = std::thread::spawn(worker_score);
    let disco = std::thread::spawn(disk_write_test);
    let cpu_score = cpu.join().unwrap_or(0);
    let disk_mbs = disco.join().unwrap_or(0);
    BenchmarkResult {
        cpu_score: cpu_score as f64,
        disk_m_bs: disk_mbs as f64,
        duration_ms: t0.elapsed().as_millis() as f64,
    }
}

/// Un hilo por core (entre 1 y 32), cada uno contando ops durante 1,8 s
/// (`workerScore`).
///
/// # La divergencia que hay aquí, y por qué
///
/// El TS mezcla con **BigInt**, y ese estado crece sin límite: tras 1000 pasos el
/// número tiene 168 bits, así que el coste por iteración sube solo. Eso ya está
/// anotado como quirk del original y el núcleo lo reproduce para el golden
/// (`BigUint`), pero **medir** con él es otra cosa: mi `BigUint` es unas mil veces
/// más lento que el BigInt de V8, así que el score salía 1 donde el TS saca 8000 y
/// el tier se caía a la patata.
///
/// Aquí se mide con `u64`, que es lo que un benchmark de CPU quiere medir, y el
/// número **no es comparable con el del TS**: el del TS depende de lo rápido que
/// sea el BigInt de su motor. Lo que sí se reproduce es la fórmula del score (con su
/// golden) y el uso que `classifyHardware` hace de él, que está topado a 10 puntos.
fn worker_score() -> u64 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
        .clamp(1, 32);
    let t0 = Instant::now();
    let handles: Vec<_> = (0..cores)
        .map(|_| {
            std::thread::spawn(|| {
                let limite = Instant::now() + Duration::from_millis(CPU_MS);
                let mut x: u64 = 3;
                let mut ops: u64 = 0;
                while Instant::now() < limite {
                    for _ in 0..20_000 {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        if x & 1 == 1 {
                            x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
                        }
                        ops += 2;
                    }
                }
                ops
            })
        })
        .collect();
    let total: u64 = handles.into_iter().filter_map(|h| h.join().ok()).sum();
    let ms = t0.elapsed().as_millis() as u64;
    cpu_score_from_ops(total, cores, ms.max(1))
}

/// Escritura secuencial de 128 MB en trozos de 4 MB, con `sync` (`diskWriteTest`).
///
/// Si algo falla devuelve 0: un disco que no deja escribir es un dato, no un error
/// que deba tumbar el benchmark.
fn disk_write_test() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("openblock-bench-{}-{nanos}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return 0;
    }
    let archivo = dir.join("seq.bin");
    let resultado = (|| -> Option<u64> {
        let mut f = std::fs::File::create(&archivo).ok()?;
        let buf = vec![0x5au8; CHUNK];
        let t0 = Instant::now();
        let mut escrito = 0usize;
        while escrito < TOTAL {
            f.write_all(&buf).ok()?;
            escrito += CHUNK;
        }
        // `fh.sync()`: sin esto se mide la caché del sistema, no el disco
        f.sync_all().ok()?;
        let secs = t0.elapsed().as_secs_f64();
        Some(disk_mbs_from_secs(TOTAL as u64, secs))
    })()
    .unwrap_or(0);
    let _ = std::fs::remove_dir_all(&dir);
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_benchmark_mide_algo() {
        // tarda ~4 s, como el del TS
        let r = run_benchmark();
        assert!(
            r.cpu_score > 0.0,
            "el score de CPU tiene que ser positivo: {r:?}"
        );
        assert!(r.duration_ms > 0.0);
        // el disco puede dar 0 si el directorio temporal no deja escribir; no se
        // exige, pero si hay número tiene que ser al menos 1 MB/s
        assert!(r.disk_m_bs == 0.0 || r.disk_m_bs >= 1.0, "{r:?}");
    }
}
