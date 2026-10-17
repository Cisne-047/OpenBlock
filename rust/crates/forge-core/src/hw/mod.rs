//! Detección de hardware y clasificación.
//!
//! `tiers` y la aritmética de `bench` son puros y viven en el núcleo. `detect` y
//! la medición del benchmark sí tocan el sistema (leer `/proc`, ejecutar
//! PowerShell, medir disco) y por eso están en el CLI: `forge-core` no puede
//! tener E/S.

pub mod bench;
pub mod tiers;
