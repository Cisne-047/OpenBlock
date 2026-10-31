//! `forge menu` — el índice interactivo: elige un número y lo ejecuta.
//!
//! Con 33 subcomandos, el problema ya no es qué puede hacer forge sino
//! recordarlo. Esto lista lo cotidiano, pide el directorio una sola vez y va
//! al subcomando —que imprime su propio uso si faltan piezas. No sustituye a
//! `help` ni a los scripts: es el "¿qué había?" de las primeras semanas.

use crate::{resolver, ExitCode};
use std::io::Write;

/// (etiqueta, comando que se lanza con <dir>)
const ACCIONES: &[(&str, &str)] = &[
    ("status — ¿está vivo? jugadores, TPS", "status"),
    ("start / stop — arrancar y apagar", "start"),
    ("console — la consola del server", "console"),
    ("watch — dashboard en vivo (Ctrl+C sale)", "watch"),
    ("logs — el log con filtros", "logs"),
    ("backup — copia de seguridad ahora", "backup"),
    ("restore — recuperar un backup", "restore"),
    ("doctor — diagnóstico + fixes", "doctor"),
    ("update — subir de build sin tocar el mundo", "update"),
    ("plugins — añadir/quitar/actualizar contenido", "plugins"),
    ("pregen — pregenerar mundo", "pregen"),
    ("world — info del mundo / trim", "world"),
    ("whitelist — quién entra", "whitelist"),
    ("op — quién manda", "op"),
    ("export — zip portable del server", "export"),
];

pub fn menu_cmd(args: &[String]) -> ExitCode {
    // el directorio se pide una vez y se reutiliza; guardado en localStorage no:
    // en el registro de forge, que para eso está
    let dir = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .cloned()
        .unwrap_or_default();
    println!("== forge · menú ==");
    println!("Elige (número, o `q` para salir). El directorio del server se pide al inicio.\n");
    for (i, (etiqueta, _)) in ACCIONES.iter().enumerate() {
        println!("  {:2}. {etiqueta}", i + 1);
    }
    print!("\nDirectorio del server (vacío = el último de `forge list`): ");
    std::io::stdout().flush().ok();
    let mut linea = String::new();
    std::io::stdin().read_line(&mut linea).ok();
    let dir = if linea.trim().is_empty() {
        if !dir.is_empty() {
            dir
        } else {
            match ultimo_del_registro() {
                Some(d) => d,
                None => {
                    eprintln!("No hay servers registrados: dime el directorio");
                    return ExitCode::from(2);
                }
            }
        }
    } else {
        linea.trim().to_string()
    };
    let raiz = resolver(&dir);
    if !raiz.join("openblock.json").exists() {
        eprintln!("No hay openblock.json en {}", raiz.display());
        return ExitCode::FAILURE;
    }
    print!("Número: ");
    std::io::stdout().flush().ok();
    let mut n = String::new();
    std::io::stdin().read_line(&mut n).ok();
    let idx: usize = match n.trim().parse::<usize>() {
        Ok(i) if i >= 1 && i <= ACCIONES.len() => i - 1,
        _ => {
            eprintln!("«{}» no es una de las opciones", n.trim());
            return ExitCode::from(2);
        }
    };
    let comando = ACCIONES[idx].1;
    println!("\n→ forge {comando} {dir}\n");
    // relanzarse a sí mismo con el subcomando elegido
    let exe = std::env::current_exe().unwrap_or_default();
    let ok = std::process::Command::new(exe)
        .arg(comando)
        .arg(&dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn ultimo_del_registro() -> Option<String> {
    crate::registry::leer().pop().map(|f| f.dir)
}
