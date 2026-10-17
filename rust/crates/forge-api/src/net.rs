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
    /// Una descarga que falló. El mensaje se escribe **tal cual**, sin prefijo: es
    /// el que ve el usuario y el mismo que da el TS.
    Descarga(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SinCurl(e) => write!(f, "no se pudo ejecutar curl: {e}"),
            Self::Red(e) => write!(f, "error de red: {e}"),
            Self::Json(e) => write!(f, "JSON inválido: {e}"),
            Self::Descarga(e) => write!(f, "{e}"),
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

// ---------- descargas a fichero ----------

/// Timeout de una descarga. El TS no pone ninguno (un `fetch` sin `AbortSignal`
/// espera lo que haga falta); aquí se pone un techo alto para que un servidor que
/// se queda colgado no cuelgue el CLI para siempre. Un JDK son ~200 MB.
const TIMEOUT_DESCARGA: u64 = 900;

/// Qué hay que verificar de lo descargado (`DownloadOpts`).
#[derive(Debug, Clone, Default)]
pub struct OpcionesDescarga<'a> {
    pub sha256: Option<&'a str>,
    pub sha512: Option<&'a str>,
    // `sha1` está en el TS y **no hace nada**: el propio código lo dice ("no
    // implementada; piston-meta ya validó el canal"). No se porta como campo para
    // no dar a entender que verifica.
}

/// Descarga a fichero con verificación de hash y **un** reintento (`downloadTo`).
///
/// El reintento devuelve el error del **primer** intento, no el del segundo: es lo
/// que hace el TS (`attempt().catch(() => { throw err })`) y cambia el mensaje que
/// ve el usuario cuando el CDN falla dos veces por motivos distintos.
pub fn download_to(
    url: &str,
    dest: &std::path::Path,
    opts: &OpcionesDescarga<'_>,
) -> Result<(), Error> {
    match intento_descarga(url, dest, opts) {
        Ok(()) => Ok(()),
        Err(primero) => {
            // flakes de CDN: medio segundo y medio, como el TS
            std::thread::sleep(std::time::Duration::from_millis(1500));
            match intento_descarga(url, dest, opts) {
                Ok(()) => Ok(()),
                Err(_) => Err(primero),
            }
        }
    }
}

fn intento_descarga(
    url: &str,
    dest: &std::path::Path,
    opts: &OpcionesDescarga<'_>,
) -> Result<(), Error> {
    use sha2::Digest;

    let destino = dest.to_string_lossy().to_string();
    let salida = Command::new("curl")
        .args([
            "-sSL", // sin `-f`: el código HTTP se lee del `-w` para poder decir cuál fue
            "--max-time",
            &TIMEOUT_DESCARGA.to_string(),
            "-A",
            crate::USER_AGENT,
            "-o",
            &destino,
            "-w",
            "%{http_code}",
            url,
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| Error::SinCurl(e.to_string()))?;

    let estado: u16 = String::from_utf8_lossy(&salida.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    if !(200..300).contains(&estado) {
        // el cuerpo del error no se queda en el destino
        let _ = std::fs::remove_file(dest);
        return Err(Error::Descarga(format!("HTTP {estado} en {url}")));
    }

    let contenido = std::fs::read(dest).map_err(|e| Error::Red(format!("{destino}: {e}")))?;
    if let Some(esperado) = opts.sha512 {
        let real = hex(&sha2::Sha512::digest(&contenido));
        if real != esperado {
            return Err(Error::Descarga(format!("SHA512 no coincide en {destino}")));
        }
    }
    if let Some(esperado) = opts.sha256 {
        let real = hex(&sha2::Sha256::digest(&contenido));
        if real != esperado {
            return Err(Error::Red(format!(
                "SHA256 no coincide en {url}: esperado {esperado}, obtenido {real}"
            )));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
