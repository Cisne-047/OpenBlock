//! HTTP mínimo para las APIs de terceros.
//!
//! Usa `curl` del sistema y no una librería: es una dependencia que ya está en
//! cualquier máquina donde se instale un server de Minecraft —los propios
//! `install.sh` que genera OpenBlock lo necesitan—, y evita meter un stack TLS
//! entero en el árbol de dependencias. Se revisa si merece la pena cuando el CLI
//! tenga que hacer muchas llamadas; para el puñado de descargas del plan, no.
//!
//! Réplica de `src/net/http.ts` en lo que importa: identificarse con User-Agent y
//! no colgarse. El reintento único del TS vive en `downloadTo`, que es para
//! ficheros; aquí no se duplica.

use std::io::Read;
use std::process::{Command, Stdio};

/// Timeout de la petición, como el que aplica el TS en `downloadTo`.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Qué fue mal, con el texto crudo de curl cuando lo hay.
#[derive(Debug)]
pub enum Error {
    /// No se pudo lanzar curl (no está instalado, por ejemplo).
    SinCurl(String),
    /// curl devolvió un error de red o HTTP.
    Red(String),
    /// La respuesta no era el JSON esperado.
    Json(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SinCurl(e) => write!(f, "no se pudo ejecutar curl: {e}"),
            Self::Red(e) => write!(f, "error de red: {e}"),
            Self::Json(e) => write!(f, "JSON inválido: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Descarga una URL y la interpreta como JSON.
///
/// `-f` para que un 404 sea error y no un cuerpo con la página de error, que es el
/// fallo clásico al consumir APIs ajenas.
pub fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, Error> {
    let txt = get_text(url)?;
    serde_json::from_str(&txt).map_err(|e| Error::Json(format!("{e} en {url}")))
}

/// Descarga una URL y devuelve el cuerpo como texto.
pub fn get_text(url: &str) -> Result<String, Error> {
    let bytes = curl(url, &[])?;
    String::from_utf8(bytes).map_err(|e| Error::Red(format!("la respuesta no es UTF-8: {e}")))
}

/// Descarga una URL y devuelve los bytes tal cual.
///
/// Es lo que necesitan los `.mrpack`: leerlos como texto los rompería en el
/// primer byte que no sea UTF-8 válido. El TS hace lo mismo con `arrayBuffer()`.
pub fn get_bytes(url: &str) -> Result<Vec<u8>, Error> {
    curl(url, &[])
}

/// Descarga una URL con cabeceras extra (la API de CurseForge pide `x-api-key`).
pub fn get_json_con_cabeceras<T: serde::de::DeserializeOwned>(
    url: &str,
    cabeceras: &[(&str, &str)],
) -> Result<T, Error> {
    let bytes = curl(url, cabeceras)?;
    let txt = String::from_utf8(bytes)
        .map_err(|e| Error::Red(format!("la respuesta no es UTF-8: {e}")))?;
    serde_json::from_str(&txt).map_err(|e| Error::Json(format!("{e} en {url}")))
}

/// Lanza curl y devuelve la salida cruda. `-f` para que un 404 sea error y no un
/// cuerpo con la página de error, que es el fallo clásico al consumir APIs ajenas.
fn curl(url: &str, cabeceras: &[(&str, &str)]) -> Result<Vec<u8>, Error> {
    let mut args: Vec<String> = vec![
        "-fsSL".to_string(), // -f: falla con HTTP >= 400; -s: sin barra; -S: errores sí; -L: sigue redirecciones
        "--max-time".to_string(),
        TIMEOUT.as_secs().to_string(),
        "-A".to_string(),
        crate::USER_AGENT.to_string(),
    ];
    for (k, v) in cabeceras {
        args.push("-H".to_string());
        args.push(format!("{k}: {v}"));
    }
    args.push(url.to_string());

    let mut hijo = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::SinCurl(e.to_string()))?;

    let mut salida = Vec::new();
    if let Some(mut out) = hijo.stdout.take() {
        out.read_to_end(&mut salida)
            .map_err(|e| Error::Red(e.to_string()))?;
    }
    let estado = hijo.wait().map_err(|e| Error::Red(e.to_string()))?;
    if !estado.success() {
        let mut err = String::new();
        if let Some(mut e) = hijo.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        return Err(Error::Red(format!(
            "curl salió con {} en {url}: {}",
            estado.code().unwrap_or(-1),
            err.trim()
        )));
    }
    Ok(salida)
}
