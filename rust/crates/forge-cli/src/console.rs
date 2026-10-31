//! `forge console <dir>` — la consola del server sin abrir otra ventana.
//!
//! Lee líneas de stdin y las manda por RCON hasta `exit`/Ctrl+D. No es un
//! attach al stdout del proceso (eso lo tiene quien lo arrancó): es lo mismo
//! que hace la consola del server, pero desde donde ya estás. El hint de
//! `say`/`list` al arrancar existe porque RCON solo devuelve la respuesta del
//! comando, no el chat que genera.

use crate::rcon::with_rcon;
use crate::status::{cargar_cfg, puerto_abierto};
use crate::ExitCode;
use std::io::{BufRead, Write};

pub fn console_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.iter().find(|a| !a.starts_with('-')) else {
        eprintln!("uso: forge console <dir>");
        return ExitCode::from(2);
    };
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if !puerto_abierto(cfg.port) {
        eprintln!(
            "El server está caído (puerto {}): arráncalo antes de abrir consola",
            cfg.port
        );
        return ExitCode::FAILURE;
    }
    if !cfg.rcon.enabled {
        eprintln!("RCON deshabilitado en la config: la consola lo necesita");
        return ExitCode::FAILURE;
    }

    println!("Consola de {} — `exit` o Ctrl+D para salir", cfg.name);
    let stdin = std::io::stdin();
    let mut fuera = std::io::stdout();
    let Ok(()) = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
        for linea in stdin.lock().lines() {
            let Ok(linea) = linea else {
                break; // Ctrl+D
            };
            let cmd = linea.trim();
            if cmd.is_empty() {
                continue;
            }
            if cmd == "exit" || cmd == "quit" {
                break;
            }
            match r.command(cmd) {
                Ok(salida) if salida.trim().is_empty() => println!("(sin salida)"),
                Ok(salida) => println!("{}", salida.trim_end()),
                Err(e) => eprintln!("RCON: {e}"),
            }
            fuera.flush().ok();
        }
        Ok(())
    }) else {
        eprintln!("No se pudo conectar por RCON (¿contraseña?)");
        return ExitCode::FAILURE;
    };
    ExitCode::SUCCESS
}
