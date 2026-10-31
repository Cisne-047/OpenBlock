//! El núcleo de OpenBlock, compilado a WASM: el constructor en el navegador.
//!
//! Es el cierre del paralelismo con DistroForge (que tiene su constructor web):
//! `forge-core` ya es puro —sin disco ni red—, así que lo único que hace falta
//! es exponerlo. La página de `web/` genera el proyecto entero (config →
//! FileMap) en el navegador y ofrece el zip: cero backend, cero cuentas, la
//! config vive en el `localStorage`. Igual que DistroForge.
//!
//! El plan (URLs de descarga) **no** se resuelve aquí: tocaría las APIs de
//! terceros y CORS lo bloquearía desde un navegador. La página genera el
//! proyecto con un plan vacío-stub… y por eso mismo la página también deja
//! pegar un `plan.json` resuelto por `forge plan` —mismo fichero, misma
//! estructura—, para que el zip salga completo.

use forge_core::plan::DownloadPlan;
use forge_core::ServerConfig;
use wasm_bindgen::prelude::*;

/// Genera el proyecto completo desde config y plan (JSON), y devuelve el
/// FileMap como JSON `{ "ruta": "contenido" }`.
#[wasm_bindgen]
pub fn generate_project_json(
    cfg_json: &str,
    plan_json: &str,
    cores: u32,
) -> Result<String, JsValue> {
    let cfg: ServerConfig = serde_json::from_str(cfg_json)
        .map_err(|e| JsValue::from_str(&format!("config inválida: {e}")))?;
    let plan: DownloadPlan = serde_json::from_str(plan_json)
        .map_err(|e| JsValue::from_str(&format!("plan inválido: {e}")))?;
    let files = forge_core::generators::generate_project(&cfg, &plan, cores);
    serde_json::to_string(&files).map_err(|e| JsValue::from_str(&format!("serializar: {e}")))
}

/// Valida una config y devuelve los errores (vacío = válida).
#[wasm_bindgen]
pub fn validate_json(cfg_json: &str) -> Result<String, JsValue> {
    let cfg: ServerConfig = serde_json::from_str(cfg_json)
        .map_err(|e| JsValue::from_str(&format!("config inválida: {e}")))?;
    let errores = forge_core::generators::validate(&cfg);
    Ok(errores.join("\n"))
}

/// La versión del núcleo, para enseñarla en la página.
#[wasm_bindgen]
pub fn version() -> String {
    forge_core::VERSION.to_string()
}
