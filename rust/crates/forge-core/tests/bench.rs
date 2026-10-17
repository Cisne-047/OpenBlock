//! Benchmark: la fórmula del score y el xorshift, contra el golden del TS.
//!
//! Los números crudos de un benchmark no son reproducibles —ni entre dos
//! ejecuciones de la misma implementación—, así que lo que se compara es la
//! aritmética que los convierte en score, y el algoritmo puro que los produce.

use forge_core::hw::bench;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn golden() -> serde_json::Value {
    let p = repo_root().join("rust/crates/forge-core/tests/golden/bench.json");
    let txt =
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("golden inválido")
}

#[test]
fn la_formula_del_score_de_cpu_coincide_con_el_ts() {
    let g = golden();
    let casos = g["cpuScore"].as_array().expect("cpuScore");
    assert!(!casos.is_empty());
    for caso in casos {
        let total: u64 = caso["totalOps"].as_str().unwrap().parse().unwrap();
        let cores = caso["cores"].as_u64().unwrap() as u32;
        let elapsed = caso["elapsedMs"].as_u64().unwrap();
        let esperado = caso["score"].as_u64().unwrap();
        assert_eq!(
            bench::cpu_score_from_ops(total, cores, elapsed),
            esperado,
            "score de ({total}, {cores} cores, {elapsed} ms)"
        );
    }
}

#[test]
fn la_formula_de_mb_por_segundo_coincide_con_el_ts() {
    let g = golden();
    for caso in g["diskMBs"].as_array().expect("diskMBs") {
        let bytes = caso["bytes"].as_u64().unwrap();
        let secs = caso["secs"].as_f64().unwrap();
        let esperado = caso["mbs"].as_u64().unwrap();
        assert_eq!(
            bench::disk_mbs_from_secs(bytes, secs),
            esperado,
            "MB/s de ({bytes} B en {secs} s)"
        );
    }
}

/// El algoritmo puro, paso a paso. Si el xorshift difiere, los dos benchmarks
/// miden cosas distintas aunque la fórmula del score sea la misma.
///
/// Y difiere de verdad si se usa `u64`: el TS usa BigInt, que crece sin límite, así
/// que tras 1000 pasos tiene 168 bits. Lo cazó la paridad y obligó a implementar
/// enteros de precisión arbitraria en el núcleo.
#[test]
fn el_xorshift_da_el_mismo_estado_final_que_el_ts() {
    let g = golden();
    let casos = g["xorshift"].as_array().expect("xorshift");
    assert!(!casos.is_empty());
    for caso in casos {
        let semilla: u64 = caso["semilla"].as_str().unwrap().parse().unwrap();
        let pasos = caso["pasos"].as_u64().unwrap();
        let esperado = caso["final"].as_str().unwrap();

        let mut x = bench::BigUint::from_u64(semilla);
        for _ in 0..pasos {
            x = bench::xorshift_step(&x);
        }
        assert_eq!(
            x.to_decimal(),
            esperado,
            "xorshift({semilla}, {pasos} pasos)"
        );
    }
}

/// El TS crece sin límite y hay que crecer con él: si esto deja de ser cierto, el
/// puerto del benchmark deja de medir lo mismo.
#[test]
fn el_estado_crece_mas_alla_de_64_bits_como_en_el_ts() {
    let mut x = bench::BigUint::from_u64(3);
    for _ in 0..1000 {
        x = bench::xorshift_step(&x);
    }
    assert!(
        x.bits() > 64,
        "tras 1000 pasos el TS ya no cabe en u64, y esto tampoco"
    );
}

/// La aritmética de precisión arbitraria, con casos que se pueden comprobar a mano.
#[test]
fn la_aritmetica_bignum_es_correcta() {
    let uno = bench::BigUint::from_u64(1);
    // desplazamientos
    assert_eq!(uno.shl(64).to_decimal(), "18446744073709551616");
    assert_eq!(bench::BigUint::from_u64(1 << 40).shr(40).to_decimal(), "1");
    assert_eq!(uno.shl(32).shr(32).to_decimal(), "1");
    assert_eq!(uno.shr(64).to_decimal(), "0");
    // xor y suma
    let a = bench::BigUint::from_u64(0b1100);
    let b = bench::BigUint::from_u64(0b1010);
    assert_eq!(a.xor(&b).to_decimal(), "6");
    assert_eq!(uno.add_u64(0).to_decimal(), "1");
    assert_eq!(
        bench::BigUint::from_u64(u64::MAX).add_u64(1).to_decimal(),
        "18446744073709551616"
    );
    // paridad
    assert!(uno.es_impar());
    assert!(!bench::BigUint::from_u64(2).es_impar());
    assert!(!bench::BigUint::from_u64(0).es_impar());
    // el cero se representa vacío
    assert_eq!(bench::BigUint::from_u64(0).to_decimal(), "0");
    assert_eq!(bench::BigUint::from_u64(0).bits(), 0);
}

#[test]
fn el_contador_de_ops_es_incondicional_como_en_el_ts() {
    // en el TS `ops += 2n` va dentro del bucle y sin condición; el `x +=` de la
    // rama mezcla el estado y no cuenta. Confundirlos cambiaría el score.
    assert_eq!(bench::OPS_POR_PASO, 2);
}
