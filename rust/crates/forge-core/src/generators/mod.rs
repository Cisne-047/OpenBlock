//! Generadores puros: cada módulo construye contenido y nada más.
//!
//! Mismo contrato que el TS: `fn(cfg) -> String` para los generadores de hoja.
//! Ninguno toca el disco, la red ni el reloj.

pub mod jvm;
pub mod ops;
pub mod paper;
pub mod properties;
pub mod service;

use crate::types::ServerConfig;

/// `server.properties` (alias del módulo, para que la llamada se lea como en TS).
pub fn server_properties(cfg: &ServerConfig) -> String {
    properties::server_properties(cfg)
}

/// GC que toca según RAM y cores (`generateProject` lo decide así en el TS).
///
/// Ojo: es la decisión del generador, no la de `recommendedGc` de `hw/tiers`
/// —el TS tiene la misma duplicidad a propósito, con entradas distintas—.
pub fn gc_for(ram_mb: u32, cores: u32) -> &'static str {
    if ram_mb >= 16 * 1024 && cores >= 8 {
        "zgc"
    } else {
        "g1"
    }
}
