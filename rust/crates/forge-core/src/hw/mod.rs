//! Detección de hardware y clasificación.
//!
//! `tiers` es puro y vive en el núcleo. `detect` y `bench` sí tocan el sistema
//! (leer `/proc`, ejecutar PowerShell, medir disco) y por eso están en el CLI:
//! `forge-core` no puede tener E/S.

pub mod tiers;
