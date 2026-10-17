//! La superficie de la CLI: qué comando se ejecuta con cada `argv`.
//!
//! El golden compara **bytes de lo que se genera**, y eso no cubre el despacho de
//! subcomandos: cuando el port se quedó sin los alias `advanced` y `smoke` que el
//! TS sí tenía, ningún test se enteró —se descubrió leyendo el README—. Esto lo
//! fija ejecutando el binario de verdad, que es la única forma de probar `main`.
//!
//! Los casos están elegidos para no tocar red ni disco: los `--help` y los errores
//! de parseo son justo lo que distingue un subcomando de otro **sin instalar nada**.
//! Ninguno puede acabar en `install` de verdad, que es lo que pasaría si el
//! `--help` dejara de cortar: por eso las aserciones también miran que la salida no
//! sea la de una instalación.

use std::process::{Command, Output};

/// Ejecuta el binario con esos argumentos. Sin entorno heredado de color: la salida
/// se compara como texto.
fn forge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("no se pudo ejecutar el binario de forge")
}

fn salida(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn codigo(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

#[test]
fn version() {
    let o = forge(&["--version"]);
    assert_eq!(codigo(&o), 0);
    assert_eq!(
        String::from_utf8_lossy(&o.stdout),
        format!("forge {}\n", forge_core::VERSION)
    );
    // el atajo corto, que es el que uno escribe
    assert_eq!(codigo(&forge(&["-V"])), 0);
}

#[test]
fn la_ayuda_sale_por_los_tres_nombres() {
    for args in [vec!["help"], vec!["--help"], vec!["-h"]] {
        let o = forge(&args);
        assert_eq!(codigo(&o), 0, "{args:?}");
        let texto = salida(&o);
        assert!(
            texto.contains("forge — forja servidores de Minecraft"),
            "{args:?} no imprimió la ayuda: {texto}"
        );
        // `guided` existe de verdad (el HELP decía que faltaba) y los alias aparecen
        // en la ayuda: una ayuda que no lista lo que hay es una ayuda que miente.
        assert!(texto.contains("forge guided"), "{args:?}");
    }
}

#[test]
fn install_help_no_instala() {
    // Sin el corte del `--help` esto se ponía a instalar con los defaults del
    // hardware: bajaba un Paper a ./server. El test lo cazaría, pero además mira
    // que no haya salido nada de una instalación.
    let o = forge(&["install", "--help"]);
    assert_eq!(codigo(&o), 0);
    let texto = salida(&o);
    assert!(
        texto.contains("forge — forja servidores de Minecraft"),
        "{texto}"
    );
    assert!(!texto.contains("== Config"), "parece que instaló: {texto}");
}

#[test]
fn advanced_es_alias_de_install() {
    // El TS daba los dos nombres a la misma rama (`case "advanced": case "install"`).
    let alias = forge(&["advanced", "--help"]);
    let directo = forge(&["install", "--help"]);
    assert_eq!(codigo(&alias), 0);
    assert_eq!(salida(&alias), salida(&directo));
}

#[test]
fn smoke_es_alias_de_self_test() {
    // Y la prueba es el error de parseo del otro, que solo lo imprime `self_test_cmd`:
    // si el alias no existiera, esto sería el `uso:` del despacho.
    let o = forge(&["smoke", "--zzz"]);
    assert_eq!(codigo(&o), 2);
    let texto = salida(&o);
    assert!(
        texto.contains("opción desconocida para self-test"),
        "«smoke» no llegó a self-test: {texto}"
    );
    assert!(!texto.contains("uso: forge <comando>"), "{texto}");
}

#[test]
fn sin_subcomando_no_se_instala_nada() {
    // El TS, sin subcomando, se iba al modo avanzado con todo `argv`. Aquí se dice
    // cómo se escribe, en vez de instalar por un flag mal escrito.
    let o = forge(&["--engine", "paper"]);
    assert_eq!(codigo(&o), 2);
    let texto = salida(&o);
    assert!(texto.contains("es una flag, no un comando"), "{texto}");
    assert!(texto.contains("forge install"), "{texto}");

    // Pero un `--help` en cualquier posición sí se atiende.
    let con_help = forge(&["--engine", "paper", "--help"]);
    assert_eq!(codigo(&con_help), 0);
    assert!(salida(&con_help).contains("forge — forja servidores de Minecraft"));
}

#[test]
fn sin_argumentos_imprime_el_uso() {
    let o = forge(&[]);
    assert_eq!(codigo(&o), 2);
    assert!(salida(&o).contains("uso: forge <comando> [opciones]"));
}

#[test]
fn comando_desconocido_imprime_el_uso() {
    let o = forge(&["no-existe"]);
    assert_eq!(codigo(&o), 2);
    assert!(salida(&o).contains("uso: forge <comando> [opciones]"));
}
