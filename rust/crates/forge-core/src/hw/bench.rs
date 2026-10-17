//! Benchmark: score de CPU multihilo y escritura secuencial. Portado de
//! `src/hw/bench.ts`.
//!
//! Solo va la **fórmula del score** a propósito: medir necesita hilos y disco, y
//! eso vive en el CLI. La fórmula sí es pura —ops, cores, milisegundos; el qué— y
//! es lo único que se puede comparar entre las dos implementaciones: los números
//! crudos de un benchmark no son reproducibles ni entre dos ejecuciones de la
//! misma implementación.
//!
//! El TS llama `cpuScore` a esto y lo usa `classifyHardware` para subir el tier
//! (hasta 10 puntos, con 7000 ops/core/ms como referencia de escritorio moderno).

/// Score de CPU del benchmark (`workerScore`, solo la parte aritmética).
///
/// El TS hace `Math.round(Number(total / BigInt(cores)) / elapsed)`: división
/// entera de BigInt (trunca), luego división real y redondeo. Aquí, integer
/// division y luego `round()`, que es lo mismo para valores positivos.
///
/// El `max(1)` de cores y de elapsed es del TS y se respeta: un benchmark no
/// puede dividir por cero solo porque la máquina sea rara.
pub fn cpu_score_from_ops(total_ops: u64, cores: u32, elapsed_ms: u64) -> u64 {
    let cores = std::cmp::max(1, cores);
    let elapsed = std::cmp::max(1, elapsed_ms);
    let por_core = total_ops / u64::from(cores);
    ((por_core as f64) / (elapsed as f64)).round() as u64
}

/// MB/s de escritura secuencial (`diskWriteTest`, solo la parte aritmética).
///
/// El TS escribe 128 MB y hace `Math.max(1, Math.round(TOTAL / 1048576 / secs))`,
/// con un mínimo de 1 MB/s para que un disco patológico no dé cero y haga pensar
/// que la medición falló.
pub fn disk_mbs_from_secs(total_bytes: u64, secs: f64) -> u64 {
    let mb = (total_bytes as f64) / 1_048_576.0;
    std::cmp::max(1, (mb / secs).round() as u64)
}

/// Tamaño de la prueba de disco, en bytes (128 MB en el TS).
pub const DISK_TEST_BYTES: u64 = 128 * 1024 * 1024;
/// Tamaño de cada escritura (4 MB en el TS).
pub const DISK_CHUNK_BYTES: usize = 4 * 1024 * 1024;
/// Milisegundos que dura el bucle de CPU de cada hilo (1800 en el TS).
pub const CPU_BENCH_MS: u64 = 1800;
/// Tope de hilos del benchmark (`Math.min(32, cores)` en el TS).
pub const MAX_BENCH_THREADS: u32 = 32;

/// Entero sin signo de precisión arbitraria, lo mínimo para el xorshift.
///
/// Existe por un hallazgo de la paridad: el TS usa **BigInt**, que crece sin
/// límite, así que `x << 13` nunca desborda. Con `u64` el resultado diverge ya en
/// el primer paso: tras 1000 pasos el TS tiene 168 bits y `u64` se queda con los 64
/// bajos. Se implementa aquí, con las cuatro operaciones que hacen falta, en vez de
/// traer una librería entera a un núcleo que quiere ser mínimo.
///
/// Los limbos van en little-endian (el índice 0 es el menos significativo) y se
/// mantienen sin ceros a la izquierda, para que `bits()` sea el tamaño real.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigUint {
    limbos: Vec<u32>,
}

impl BigUint {
    /// Desde un `u64`.
    pub fn from_u64(v: u64) -> Self {
        let mut limbos = vec![(v & 0xFFFF_FFFF) as u32, (v >> 32) as u32];
        limbos.truncate(Self::limbos_utiles(&limbos));
        Self { limbos }
    }

    fn limbos_utiles(limbos: &[u32]) -> usize {
        limbos.iter().rposition(|&l| l != 0).map_or(0, |i| i + 1)
    }

    fn normalizar(&mut self) {
        let n = Self::limbos_utiles(&self.limbos);
        self.limbos.truncate(n);
    }

    /// Número de bits significativos.
    pub fn bits(&self) -> usize {
        match self.limbos.last() {
            None => 0,
            Some(&alto) => (self.limbos.len() - 1) * 32 + (32 - alto.leading_zeros() as usize),
        }
    }

    /// Desplazamiento a la izquierda (`x << n`).
    pub fn shl(&self, n: usize) -> Self {
        if self.limbos.is_empty() || n == 0 {
            return self.clone();
        }
        let limbos_enteros = n / 32;
        let bits = n % 32;
        let mut out = vec![0u32; limbos_enteros];
        if bits == 0 {
            out.extend_from_slice(&self.limbos);
        } else {
            let mut acarreo = 0u32;
            for &l in &self.limbos {
                out.push((l << bits) | acarreo);
                acarreo = l >> (32 - bits);
            }
            if acarreo != 0 {
                out.push(acarreo);
            }
        }
        let mut r = Self { limbos: out };
        r.normalizar();
        r
    }

    /// Desplazamiento a la derecha (`x >> n`).
    pub fn shr(&self, n: usize) -> Self {
        let limbos_enteros = n / 32;
        if limbos_enteros >= self.limbos.len() {
            return Self { limbos: Vec::new() };
        }
        let bits = n % 32;
        let resto = &self.limbos[limbos_enteros..];
        let mut out = Vec::with_capacity(resto.len());
        if bits == 0 {
            out.extend_from_slice(resto);
        } else {
            for i in 0..resto.len() {
                let bajo = resto[i] >> bits;
                let alto = if i + 1 < resto.len() {
                    resto[i + 1] << (32 - bits)
                } else {
                    0
                };
                out.push(bajo | alto);
            }
        }
        let mut r = Self { limbos: out };
        r.normalizar();
        r
    }

    /// XOR bit a bit (`x ^ y`).
    pub fn xor(&self, otro: &Self) -> Self {
        let n = std::cmp::max(self.limbos.len(), otro.limbos.len());
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let a = self.limbos.get(i).copied().unwrap_or(0);
            let b = otro.limbos.get(i).copied().unwrap_or(0);
            out.push(a ^ b);
        }
        let mut r = Self { limbos: out };
        r.normalizar();
        r
    }

    /// Suma un `u64` y devuelve el resultado (`x += constante`).
    pub fn add_u64(&self, sumando: u64) -> Self {
        let mut out = self.limbos.clone();
        let mut acarreo = sumando as u128;
        let mut i = 0;
        while acarreo != 0 {
            if i == out.len() {
                out.push(0);
            }
            let total = out[i] as u128 + (acarreo & 0xFFFF_FFFF);
            out[i] = (total & 0xFFFF_FFFF) as u32;
            acarreo = (acarreo >> 32) + (total >> 32);
            i += 1;
        }
        let mut r = Self { limbos: out };
        r.normalizar();
        r
    }

    /// Bit menos significativo (`x & 1n`).
    pub fn es_impar(&self) -> bool {
        self.limbos.first().is_some_and(|l| l & 1 == 1)
    }

    /// A decimal, sin signo. División larga por 10 sobre los limbos.
    pub fn to_decimal(&self) -> String {
        if self.limbos.is_empty() {
            return "0".to_string();
        }
        let mut copia = self.limbos.clone();
        let mut digitos = Vec::new();
        while !copia.is_empty() {
            let mut resto: u64 = 0;
            // de arriba abajo: es la división escolar por 10
            for limbo in copia.iter_mut().rev() {
                let actual = (resto << 32) | u64::from(*limbo);
                *limbo = (actual / 10) as u32;
                resto = actual % 10;
            }
            digitos.push(b'0' + resto as u8);
            let n = Self::limbos_utiles(&copia);
            copia.truncate(n);
        }
        digitos.reverse();
        String::from_utf8(digitos).expect("son dígitos ASCII")
    }
}

/// Un paso del xorshift que mide el benchmark (`WORKER_SRC` del TS).
///
/// Con precisión arbitraria, porque el TS usa BigInt y crece sin límite: ver
/// [`BigUint`]. El TS lo hace con `BigInt` sin signo de facto —solo desplaza,
/// enmascara y suma—, así que el valor final coincide paso a paso.
///
/// Ojo con el contador, que es lo que mide el score: en el TS `ops += 2n` es
/// **incondicional**, dentro del bucle, y el `x +=` de la rama es otra cosa
/// (mezcla el estado, no cuenta). Confundirlos cambiaría el score.
pub fn xorshift_step(x: &BigUint) -> BigUint {
    let mut x = x.xor(&x.shl(13));
    x = x.xor(&x.shr(7));
    x = x.xor(&x.shl(17));
    if x.es_impar() {
        x = x.add_u64(0x9e37_79b9_7f4a_7c15);
    }
    x
}

/// Operaciones que cuenta cada paso del bucle, incondicional (`ops += 2n`).
pub const OPS_POR_PASO: u64 = 2;
