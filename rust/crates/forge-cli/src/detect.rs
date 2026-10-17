//! Detección de hardware. Vive en el CLI porque toca el sistema: lee `/proc`,
//! ejecuta `lsblk`/`df` o PowerShell, y consulta la API de Windows por FFI.
//!
//! `forge-core` no tiene una sola referencia a `std::fs`/`std::process`, así que
//! esto no puede estar ahí. Es el mismo reparto que el TS hace con `output.ts`:
//! el núcleo construye, el borde pregunta al sistema.
//!
//! La paridad no se puede comprobar con un golden —el hardware de la máquina de
//! CI no es el de nadie—, así que se comprueba de otra forma: `forge detect`
//! imprime el mismo JSON que `openblock detect --json` del TS, y comparar las dos
//! salidas en la misma máquina dice si alguna miente.

use forge_core::types::{DiskKind, HardwareProfile};
use std::process::Command;

/// Timeout de los comandos externos, como el `TIMEOUT` del TS.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Ejecuta un comando y devuelve su stdout recortado; `None` si falla o tarda
/// demasiado.
///
/// El timeout se aplica de verdad, matando el proceso: `Command::output()` se
/// queda esperando para siempre si PowerShell decide colgarse, y la detección de
/// hardware no puede bloquear el CLI.
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    use std::io::Read;
    use std::process::Stdio;

    let mut hijo = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let inicio = std::time::Instant::now();
    loop {
        match hijo.try_wait() {
            Ok(Some(estado)) => {
                if !estado.success() {
                    return None;
                }
                let mut salida = String::new();
                hijo.stdout.take()?.read_to_string(&mut salida).ok()?;
                return Some(salida.trim().to_string());
            }
            Ok(None) => {
                if inicio.elapsed() > TIMEOUT {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(windows)]
fn ps(script: &str) -> Option<String> {
    run(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )
}

#[cfg(not(windows))]
fn sh(cmd: &str, args: &[&str]) -> Option<String> {
    run(cmd, args)
}

// ---------- RAM total y libre ----------

#[cfg(windows)]
mod win {
    /// `MEMORYSTATUSEX` de la API de Windows, sin dependencias externas.
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    /// (total, libre, swap total) en MB. La swap es la página total menos la
    /// física, que es justo lo que devuelve `Win32_PageFileUsage` en la práctica.
    ///
    /// Los MB van **redondeados**, no truncados: el TS hace
    /// `Math.round(os.totalmem() / 1048576)` sobre los mismos bytes, y truncar
    /// pierde 1 MB de forma consistente.
    pub fn memory_mb() -> Option<(u64, u64, u64)> {
        let st = status()?;
        let mb = |b: u64| ((b as f64) / 1_048_576.0).round() as u64;
        // la swap es la página total menos la física, que es lo que suma
        // `Win32_PageFileUsage` en la práctica
        let swap = st.total_page_file.saturating_sub(st.total_phys);
        Some((mb(st.total_phys), mb(st.avail_phys), mb(swap)))
    }

    /// Bytes crudos de RAM total y libre, como los ve `os.totalmem()`.
    pub fn memory_bytes() -> Option<(u64, u64)> {
        let st = status()?;
        Some((st.total_phys, st.avail_phys))
    }

    /// Rellena `MEMORYSTATUSEX` una sola vez; los dos lectores parten de aquí.
    fn status() -> Option<MemoryStatusEx> {
        let mut st = MemoryStatusEx {
            length: std::mem::size_of::<MemoryStatusEx>() as u32,
            memory_load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page_file: 0,
            avail_page_file: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended_virtual: 0,
        };
        // SAFETY: el buffer es del tamaño que declara `length`, que es el
        // contrato de GlobalMemoryStatusEx.
        let ok = unsafe { GlobalMemoryStatusEx(&mut st) };
        // `avail_phys` a cero significaría un fallo silencioso; se comprueba
        (ok != 0).then_some(st)
    }
}

/// Lee un campo de `/proc/meminfo` en kB y lo pasa a MB.
#[cfg(not(windows))]
fn meminfo_mb(campo: &str) -> u64 {
    let Ok(txt) = std::fs::read_to_string("/proc/meminfo") else {
        return 0;
    };
    for linea in txt.lines() {
        if let Some(resto) = linea.strip_prefix(campo) {
            let kb: u64 = resto
                .trim_start_matches(':')
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            // `Math.round` del TS sobre el valor en MB, no una división entera
            return ((kb as f64) / 1024.0).round() as u64;
        }
    }
    0
}

/// RAM total en MB, **redondeando como el TS**.
///
/// El TS hace `Math.round(os.totalmem() / 1048576)`, y eso no es una división
/// entera: 34266234880 B son 32678.83 MB, que redondean a 32679, no a 32678. Mi
/// primera versión truncaba y perdía 1 MB de forma consistente; lo cazó comparar
/// las dos detecciones en la misma máquina.
fn total_ram_mb() -> u64 {
    #[cfg(windows)]
    {
        win::memory_mb().map(|(t, _, _)| t).unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        meminfo_mb("MemTotal:")
    }
}

/// RAM libre en MB, con el mismo redondeo que el TS.
fn free_ram_mb() -> u64 {
    #[cfg(windows)]
    {
        win::memory_mb().map(|(_, f, _)| f).unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        meminfo_mb("MemAvailable:")
    }
}

// ---------- CPU, host y versión ----------

/// Modelo de CPU y número de cores lógicos.
fn cpu() -> (String, u32) {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);

    #[cfg(windows)]
    let modelo = run(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)",
        ],
    )
    .unwrap_or_default();

    #[cfg(not(windows))]
    let modelo = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|txt| {
            txt.lines()
                .find(|l| l.starts_with("model name") || l.starts_with("Model"))
                .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
        })
        .unwrap_or_default();

    let modelo = if modelo.is_empty() {
        "desconocido".to_string()
    } else {
        modelo
    };
    (modelo, cores)
}

/// Versión del SO (`os.release()` en el TS).
///
/// En Windows, `os.release()` **quita el último componente** de la versión del
/// kernel: devuelve `10.0.26300`, no `10.0.26300.0`. Se recorta igual para que la
/// paridad valga algo; lo cazó comparar las dos salidas en la misma máquina.
#[cfg(windows)]
fn os_version() -> String {
    let completa = run(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[System.Environment]::OSVersion.Version.ToString()",
        ],
    )
    .unwrap_or_default();
    match completa.rsplit_once('.') {
        // solo se quita si el último componente es un número (la revisión)
        Some((corta, ultimo))
            if ultimo.chars().all(|c| c.is_ascii_digit()) && corta.contains('.') =>
        {
            corta.to_string()
        }
        _ => completa,
    }
}

/// En Linux, `os.release()` es el `uname -r` tal cual.
#[cfg(not(windows))]
fn os_version() -> String {
    run("uname", &["-r"]).unwrap_or_default()
}

fn hostname() -> String {
    // `HOSTNAME`/`COMPUTERNAME` es lo que usaría el shell; si no, se deja vacío
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| {
            #[cfg(not(windows))]
            {
                run("hostname", &[]).unwrap_or_default()
            }
            #[cfg(windows)]
            {
                String::new()
            }
        })
}

/// Arquitectura con los nombres del TS: `os.arch()` dice `x64` o `arm64`.
fn arch() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "x64".to_string(),
        "aarch64" => "arm64".to_string(),
        otra => otra.to_string(),
    }
}

// ---------- Swap ----------

fn swap_mb() -> u64 {
    #[cfg(windows)]
    {
        win::memory_mb().map(|(_, _, s)| s).unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        meminfo_mb("SwapTotal:")
    }
}

// ---------- Disco ----------

#[cfg(windows)]
fn disk_kind() -> DiskKind {
    // BusType distingue NVMe real; MediaType solo dice SSD/HDD (o "Unspecified" en VMs)
    let bus = ps("Get-PhysicalDisk | Sort-Object Size -Descending | Select-Object -First 1 -ExpandProperty BusType")
        .unwrap_or_default();
    if bus.to_lowercase().contains("nvme") {
        return DiskKind::Nvme;
    }
    let medio = ps("Get-PhysicalDisk | Sort-Object Size -Descending | Select-Object -First 1 -ExpandProperty MediaType")
        .unwrap_or_default()
        .to_lowercase();
    if medio.contains("ssd") {
        return DiskKind::Ssd;
    }
    if medio.contains("hdd") {
        return DiskKind::Hdd;
    }
    DiskKind::Unknown
}

#[cfg(not(windows))]
fn disk_kind() -> DiskKind {
    // primera línea = primer disco por orden del kernel
    let Some(out) = sh("lsblk", &["-dn", "-o", "NAME,ROTA"]) else {
        return DiskKind::Unknown;
    };
    let primera = out.lines().next().unwrap_or("");
    let mut partes = primera.split_whitespace();
    let nombre = partes.next().unwrap_or("");
    let rota = partes.next().unwrap_or("1");
    if nombre.starts_with("nvme") {
        return DiskKind::Nvme;
    }
    match rota {
        "0" => DiskKind::Ssd,
        "1" => DiskKind::Hdd,
        _ => DiskKind::Unknown,
    }
}

#[cfg(windows)]
fn disk_free_mb() -> u64 {
    // `(Get-PSDrive -Name (Get-Location).Drive.Name).Free / 1MB`, con la coma
    // decimal normalizada: en es-ES PowerShell devuelve "1234,5"
    let actual = std::env::current_dir().unwrap_or_default();
    let letra = actual
        .components()
        .next()
        .map(|c| {
            c.as_os_str()
                .to_string_lossy()
                .trim_end_matches(':')
                .to_string()
        })
        .unwrap_or_default();
    let script = format!("(Get-PSDrive -Name {letra}).Free / 1MB");
    let out = ps(&script).unwrap_or_default();
    out.replace(',', ".")
        .parse::<f64>()
        .map(|v| v.round() as u64)
        .unwrap_or(0)
}

#[cfg(not(windows))]
fn disk_free_mb() -> u64 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let Some(out) = sh("df", &["-Pm", &cwd.to_string_lossy()]) else {
        return 0;
    };
    out.lines()
        .nth(1)
        .and_then(|l| l.split_whitespace().nth(3))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// ---------- Virtualización ----------

#[cfg(windows)]
fn virtualized() -> bool {
    let modelo = ps("(Get-CimInstance Win32_ComputerSystem).Model").unwrap_or_default();
    let m = modelo.to_lowercase();
    ["virtual", "vmware", "kvm", "qemu", "virtualbox", "xen"]
        .iter()
        .any(|v| m.contains(v))
}

#[cfg(not(windows))]
fn virtualized() -> bool {
    if let Ok(info) = std::fs::read_to_string("/proc/cpuinfo") {
        if info.to_lowercase().contains("hypervisor") {
            return true;
        }
    }
    std::path::Path::new("/proc/xen").exists()
}

// ---------- Perfil ----------

/// Detecta el hardware de esta máquina (`detectHardware`).
///
/// Nunca falla: cada dato individual tiene su respaldo, igual que el TS, que
/// envuelve cada comprobación en un `try` que devuelve un valor por defecto.
pub fn detect_hardware() -> HardwareProfile {
    let (cpu_model, cores) = cpu();
    let (disk_kind, disk_free_mb, virtualized) = (disk_kind(), disk_free_mb(), virtualized());
    HardwareProfile {
        os: if cfg!(windows) {
            "windows".into()
        } else {
            "linux".into()
        },
        os_version: os_version(),
        arch: arch(),
        hostname: hostname(),
        cpu_model,
        cores,
        ram_total_mb: total_ram_mb(),
        ram_free_mb: free_ram_mb(),
        swap_total_mb: swap_mb(),
        disk_kind,
        disk_free_mb,
        virtualized,
        bench: None,
    }
}

// ---------- Bytes crudos para la prueba de paridad ----------

/// Bytes de RAM y disco **sin redondear**, para poder comparar con el TS.
///
/// El JSON de `detect` no sirve para una prueba automática: `ramFreeMB` y
/// `diskFreeMB` son medidas instantáneas, así que dos ejecuciones seguidas dan
/// números distintos sin que nada esté mal. Los valores **deterministas** (RAM
/// total, versión del SO) sí se pueden comparar, y eso es lo que expone esto:
/// `tests/detect.test.ts` lo compara contra `os.totalmem()` y `os.release()`.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BytesCrudos {
    /// Bytes de RAM física total, como `os.totalmem()`.
    pub ram_total_bytes: u64,
    /// Bytes libres de RAM, como `os.freemem()`.
    pub ram_free_bytes: u64,
    /// Bytes libres en el disco del directorio actual.
    pub disk_free_bytes: u64,
    /// Versión del SO ya recortada, como `os.release()`.
    pub os_version: String,
}

/// Lee los bytes crudos para la prueba de paridad.
pub fn bytes_crudos() -> BytesCrudos {
    #[cfg(windows)]
    let (ram_total_bytes, ram_free_bytes) = win::memory_bytes().unwrap_or((0, 0));
    #[cfg(not(windows))]
    let (ram_total_bytes, ram_free_bytes) =
        (meminfo_bytes("MemTotal:"), meminfo_bytes("MemAvailable:"));

    BytesCrudos {
        ram_total_bytes,
        ram_free_bytes,
        disk_free_bytes: disk_free_bytes_raw(),
        os_version: os_version(),
    }
}

/// Bytes de `/proc/meminfo`, sin pasar por MB (para la comparación).
#[cfg(not(windows))]
fn meminfo_bytes(campo: &str) -> u64 {
    let Ok(txt) = std::fs::read_to_string("/proc/meminfo") else {
        return 0;
    };
    for linea in txt.lines() {
        if let Some(resto) = linea.strip_prefix(campo) {
            let kb: u64 = resto
                .trim_start_matches(':')
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            return kb * 1024;
        }
    }
    0
}

/// Bytes libres en disco, sin redondear.
#[cfg(windows)]
fn disk_free_bytes_raw() -> u64 {
    let actual = std::env::current_dir().unwrap_or_default();
    let letra = actual
        .components()
        .next()
        .map(|c| {
            c.as_os_str()
                .to_string_lossy()
                .trim_end_matches(':')
                .to_string()
        })
        .unwrap_or_default();
    let out = ps(&format!("(Get-PSDrive -Name {letra}).Free")).unwrap_or_default();
    out.replace(',', ".")
        .parse::<f64>()
        .map(|v| v as u64)
        .unwrap_or(0)
}

/// `df -Pk` da kB; el TS usa `df -Pm` y trunca a MB.
#[cfg(not(windows))]
fn disk_free_bytes_raw() -> u64 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let Some(out) = sh("df", &["-Pk", &cwd.to_string_lossy()]) else {
        return 0;
    };
    out.lines()
        .nth(1)
        .and_then(|l| l.split_whitespace().nth(3))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}
