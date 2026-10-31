//! `forge doctor <dir> --schedule` — el doctor, programado.
//!
//! Un doctor que solo se pasa cuando algo va mal diagnostica tarde. Esto
//! escribe en el directorio del server un timer+service de systemd (Linux) o
//! imprime el comando de Task Scheduler (Windows) para correr el doctor
//! semanal: `--yes` incluido, porque un diagnóstico programado que se pone a
//! preguntar no termina nunca. Los ficheros quedan en el directorio, no en
//! /etc: activarlos es un comando que el usuario ejecuta con sudo a propósito.

use crate::status::cargar_cfg;
use crate::{resolver, ExitCode};
use std::fs;

pub fn schedule_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge doctor <dir> --schedule");
        return ExitCode::from(2);
    };
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let raiz = resolver(dir);
    let abs = raiz.to_string_lossy().replace('\\', "/");

    if cfg!(windows) {
        println!("Windows: crea la tarea con (PowerShell de administrador):");
        println!(
            "  schtasks /Create /SC WEEKLY /D SUN /ST 10:00 /TN \"OpenBlock doctor {name}\" /TR \"forge doctor {abs} --yes\"",
            name = cfg.name
        );
        return ExitCode::SUCCESS;
    }

    let service = format!(
        "[Unit]\nDescription=OpenBlock doctor de {name}\n\n[Service]\nType=oneshot\nWorkingDirectory={dir}\nExecStart=forge doctor {dir} --yes\n",
        name = cfg.name,
        dir = abs
    );
    let timer = "[Unit]\nDescription=Doctor semanal de OpenBlock\n\n[Timer]\nOnCalendar=Sun 10:00\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n";
    fs::write(raiz.join("openblock-doctor.service"), service)
        .and_then(|_| fs::write(raiz.join("openblock-doctor.timer"), timer))
        .expect("escribir units");
    println!(
        "✓ openblock-doctor.service/.timer escritos en {}",
        raiz.display()
    );
    println!("activar (una vez, con sudo):");
    println!("  sudo cp {abs}/openblock-doctor.* /etc/systemd/system/ && sudo systemctl enable --now openblock-doctor.timer");
    ExitCode::SUCCESS
}
