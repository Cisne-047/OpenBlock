//! Resolución contra las APIs de terceros, portado de `src/api/`.
//!
//! Este crate **sí toca la red**, así que no puede vivir en `forge-core`: el
//! reparto es el mismo que en el TS, donde `fetch` solo aparece en `api/` y
//! `net/`, y los generadores son puros.
//!
//! La parte que sí es pura —parsear y filtrar las respuestas— va en funciones
//! separadas y se compara con el TS. Lo que no se puede comparar es la respuesta
//! de una API ajena, que cambia sin avisar.

pub mod mojang;
pub mod net;

/// Identificador con el que nos presentamos a las APIs de terceros.
///
/// Sale del `Cargo.toml`, igual que en `forge-core`, así que no puede
/// desincronizarse del `package.json` del TS.
pub const USER_AGENT: &str = concat!(
    "OpenBlock/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/cisne-047)"
);
