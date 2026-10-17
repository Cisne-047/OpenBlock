//! El self-test: instala, arranca y diagnostica un server de verdad, portado de
//! `src/selftest.ts`.
//!
//! Es el mismo que usa el CI del TS, y por eso importa que el veredicto sea el
//! mismo: un verificador que dice «OK» cuando no lo está es peor que no tenerlo.

use forge_core::doctor::{evaluar_self_test, Paso};
use forge_core::types::{
    BackupConfig, ModpackKind, ModpackRef, RconConfig, ServerConfig, ServiceConfig, TargetOs,
};
use std::path::PathBuf;

/// Opciones del self-test (`SelfTestOptions`).
#[derive(Debug, Clone, Default)]
pub struct OpcionesSelfTest {
    pub mc: Option<String>,
    pub port: Option<u16>,
    pub cores: u32,
    /// no borra el directorio al terminar
    pub keep: bool,
}

/// La config del smoke test (`selfTestConfig`).
///
/// Va fija a propósito: el self-test mide que el ciclo funcione, no el hardware, y
/// una config que cambie con la máquina dejaría de comparar lo mismo. Sin
/// pregeneración (mide arranque, no rendimiento) y sin backup ni servicio.
pub fn self_test_config(dir: &str, opts: &OpcionesSelfTest) -> ServerConfig {
    let puerto = opts.port.unwrap_or(25599);
    let mc = opts.mc.clone().unwrap_or_else(|| "1.20.4".to_string());
    ServerConfig {
        name: "openblock-smoke".to_string(),
        mc_version: mc,
        engine: forge_core::types::EngineId::Paper,
        server_dir: dir.to_string(),
        ram_mb: 2048,
        players: 5,
        view_distance: 5,
        simulation_distance: 5,
        port: puerto,
        motd: "openblock self-test".to_string(),
        difficulty: forge_core::types::Difficulty::Peaceful,
        gamemode: forge_core::types::Gamemode::Survival,
        world_type: forge_core::types::WorldType::Flat,
        seed: "openblock".to_string(),
        online_mode: false,
        whitelist: false,
        pvp: false,
        hardcore: false,
        modpack: ModpackRef {
            kind: ModpackKind::None,
            url: String::new(),
        },
        performance: forge_core::types::PerfProfile::Balanced,
        pregen_radius: 0,
        backup: BackupConfig {
            enabled: false,
            interval_hours: 24,
            keep: 1,
        },
        service: ServiceConfig {
            enabled: false,
            user: None,
        },
        rcon: RconConfig {
            enabled: true,
            password: "openblock-smoke-test".to_string(),
            port: puerto + 10,
        },
        chunkyborder: false,
        target_os: if cfg!(windows) {
            TargetOs::Windows
        } else {
            TargetOs::Linux
        },
        java_path: None,
        build: None,
        loader_build: None,
        installer_build: None,
        custom_items: Vec::new(),
    }
}

/// Añade un paso al veredicto y lo imprime.
///
/// Libre y no un closure: un `push` capturaría `log`, y `apply_config` también lo
/// necesita prestado. Dos closures pidiendo `&mut` a la vez no compilan, y con
/// razón.
fn empujar(
    pasos: &mut Vec<Paso>,
    log: &mut dyn FnMut(String),
    name: &str,
    ok: bool,
    detail: String,
) {
    log(format!(
        "  {} {name} — {detail}",
        if ok { "✓" } else { "✗" }
    ));
    pasos.push(Paso {
        name: name.to_string(),
        ok,
        detail,
    });
}

/// El resultado del self-test (`SelfTestResult`).
#[derive(Debug, Clone)]
pub struct ResultadoSelfTest {
    pub ok: bool,
    pub pasos: Vec<Paso>,
}

/// Un directorio temporal propio, sin traer `tempfile`.
///
/// `mkdtemp` de Node crea el directorio y devuelve la ruta; aquí el nombre sale del
/// reloj y del pid, que para un smoke test es suficiente y no colisiona.
fn dir_temporal() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("openblock-smoke-{}-{nanos}", std::process::id()))
}

/// Instala, arranca y diagnostica (`selfTest`).
pub fn self_test(
    opts: &OpcionesSelfTest,
    log: &mut dyn FnMut(String),
) -> Result<ResultadoSelfTest, String> {
    let dir = dir_temporal();
    let mut pasos: Vec<Paso> = Vec::new();

    let resultado = (|| -> Result<(), String> {
        let mut cfg = self_test_config(&dir.to_string_lossy(), opts);
        log(format!(
            "== Self-test: Paper {} en {} ==",
            cfg.mc_version,
            dir.display()
        ));

        // 1) instalación real: resuelve Java, escribe el proyecto y baja el jar
        let apply_opts = crate::apply::OpcionesApply {
            cores: opts.cores,
            ..Default::default()
        };
        crate::apply::apply_config(&mut cfg, &apply_opts, log)?;
        empujar(
            &mut pasos,
            log,
            "instalación",
            true,
            format!("Paper {}, {} hilos", cfg.mc_version, opts.cores),
        );

        let eula = dir.join("eula.txt").exists();
        empujar(
            &mut pasos,
            log,
            "eula aceptado",
            eula,
            "eula.txt".to_string(),
        );

        let jars: Vec<String> = std::fs::read_dir(&dir)
            .map(|e| {
                e.flatten()
                    .map(|x| x.file_name().to_string_lossy().to_string())
                    .filter(|n| n.ends_with(".jar"))
                    .collect()
            })
            .unwrap_or_default();
        empujar(
            &mut pasos,
            log,
            "jar del engine descargado",
            !jars.is_empty(),
            if jars.is_empty() {
                "ningún .jar en el directorio".to_string()
            } else {
                jars.join(", ")
            },
        );

        // 2) diagnóstico con dry run REAL: arranca, espera Done y apaga por RCON.
        // En solo lectura, para que el doctor no arregle lo que tiene que encontrar.
        let informe =
            crate::doctor::ejecutar(&mut cfg, &dir, true, crate::doctor::Modo::SoloLectura);

        // 3) el veredicto, que es lo que decide si esto pasa
        for paso in evaluar_self_test(&informe) {
            empujar(&mut pasos, log, &paso.name, paso.ok, paso.detail);
        }
        Ok(())
    })();

    // el `finally`: el directorio se borra salvo que se pida conservarlo
    if !opts.keep {
        let _ = std::fs::remove_dir_all(&dir);
    } else {
        log(format!("  Directorio conservado: {}", dir.display()));
    }
    resultado?;

    let ok = pasos.iter().all(|p| p.ok);
    Ok(ResultadoSelfTest { ok, pasos })
}

/// Ejecuta el self-test e imprime el veredicto (`runSelfTest`). Devuelve el código.
pub fn run_self_test(opts: &OpcionesSelfTest) -> i32 {
    match self_test(opts, &mut |l| println!("{l}")) {
        Ok(r) if r.ok => {
            println!("\n✓ Self-test OK: instala, arranca y diagnostica.\n");
            0
        }
        Ok(r) => {
            let fallos: Vec<&str> = r
                .pasos
                .iter()
                .filter(|p| !p.ok)
                .map(|p| p.name.as_str())
                .collect();
            eprintln!("\n✗ Self-test FALLÓ: {}\n", fallos.join(", "));
            1
        }
        Err(e) => {
            eprintln!("\n✗ Self-test FALLÓ: {e}\n");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::doctor::Informe;

    #[test]
    fn la_config_del_smoke_test_es_la_del_ts() {
        let cfg = self_test_config("/tmp/x", &OpcionesSelfTest::default());
        assert_eq!(cfg.name, "openblock-smoke");
        assert_eq!(cfg.mc_version, "1.20.4");
        assert_eq!(cfg.port, 25599);
        // el puerto de RCON va 10 por encima del del juego
        assert_eq!(cfg.rcon.port, 25609);
        assert_eq!(cfg.ram_mb, 2048);
        assert_eq!(cfg.difficulty, forge_core::types::Difficulty::Peaceful);
        assert_eq!(cfg.world_type, forge_core::types::WorldType::Flat);
        assert!(!cfg.online_mode);
        // sin pregeneración: el smoke test mide arranque, no rendimiento
        assert_eq!(cfg.pregen_radius, 0);
        assert!(!cfg.backup.enabled && !cfg.service.enabled);
        assert!(cfg.rcon.enabled);
        assert_eq!(
            cfg.rcon.password, "openblock-smoke-test",
            "el self-test no puede depender de una contraseña aleatoria"
        );
        // y la config pasa la validación del generador
        assert!(forge_core::generators::validate(&cfg).is_empty());
    }

    #[test]
    fn el_puerto_y_la_version_se_pueden_cambiar() {
        let cfg = self_test_config(
            "/tmp/x",
            &OpcionesSelfTest {
                mc: Some("1.21.4".to_string()),
                port: Some(26000),
                ..Default::default()
            },
        );
        assert_eq!(cfg.mc_version, "1.21.4");
        assert_eq!(cfg.port, 26000);
        assert_eq!(cfg.rcon.port, 26010);
    }

    #[test]
    fn el_veredicto_de_un_informe_limpio_pasa_todo() {
        // el golden del núcleo cubre los casos malos; aquí solo se comprueba que un
        // informe limpio pasa los cuatro pasos
        let informe = Informe {
            findings: Vec::new(),
            applied: Vec::new(),
            pending: Vec::new(),
            score: 100,
            grade: forge_core::doctor::nota(100),
            boot_tested: true,
            boot_ok: true,
        };
        let pasos = evaluar_self_test(&informe);
        assert_eq!(pasos.len(), 4);
        assert!(pasos.iter().all(|p| p.ok), "{pasos:?}");
    }
}
