//! `forge whitelist` y `forge op` — añadir gente sin editar JSON a mano.
//!
//! Editar `whitelist.json`/`ops.json` a mano es el clásico que rompe servers:
//! UUID mal copiado, coma que falta. Esto resuelve el UUID (Mojang si hay
//! online-mode; offline v3 si no), escribe el JSON bonito, y si el server está
//! arriba también se lo dice por RCON para que lo aplique al vuelo.
//!
//! Con el server caído se edita el fichero y ya está: al arrancar lo carga.
//! Con el server arriba se hace por RCON (que además persiste él solo) y el
//! fichero local se refresca para que no se desincronicen.

use crate::rcon::with_rcon;
use crate::status::{cargar_cfg, puerto_abierto};
use crate::{resolver, ExitCode};
use forge_core::ServerConfig;
use md5::{Digest, Md5};
use serde_json::{json, Value};
use std::fs;
use std::net::TcpStream;
use std::time::Duration;

/// Un jugador en whitelist.json/ops.json.
#[derive(Clone, PartialEq, Eq)]
struct Entrada {
    uuid: String,
    nombre: String,
    level: Option<u8>, // solo ops
}

/// UUID online: se lo pregunta a Mojang y le pone los guiones.
fn uuid_mojang(nombre: &str) -> Result<String, String> {
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{}", nombre);
    let v: Value = forge_api::net::get_json(&url).map_err(|e| e.to_string())?;
    let id = v["id"]
        .as_str()
        .ok_or_else(|| format!("Mojang no conoce a «{nombre}» (¿bien escrito? ¿es premium?)"))?;
    Ok(con_guiones(id))
}

/// 32 hex sin guiones → 8-4-4-4-12 (tolera que ya vengan).
fn con_guiones(id: &str) -> String {
    let h: String = id.chars().filter(|c| *c != '-').collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// UUID offline: MD5 de "OfflinePlayer:<nombre>" con los bits de versión 3 y
/// variante RFC 4122 forzados. Es el cálculo exacto que hace el server vanilla.
fn uuid_offline(nombre: &str) -> String {
    let mut h = Md5::new();
    h.update(format!("OfflinePlayer:{nombre}"));
    let mut d = h.finalize();
    d[6] = (d[6] & 0x0f) | 0x30; // versión 3
    d[8] = (d[8] & 0x3f) | 0x80; // variante RFC 4122
    let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn uuid_para(cfg: &ServerConfig, nombre: &str) -> Result<String, String> {
    if cfg.online_mode {
        uuid_mojang(nombre)
    } else {
        Ok(uuid_offline(nombre))
    }
}

/// Lee whitelist.json u ops.json; fichero ausente = lista vacía, no error.
fn leer_lista(dir: &std::path::Path, fichero: &str) -> Result<Vec<Entrada>, String> {
    let p = dir.join(fichero);
    let Ok(txt) = fs::read_to_string(&p) else {
        return Ok(Vec::new());
    };
    if txt.trim().is_empty() {
        return Ok(Vec::new());
    }
    let v: Value =
        serde_json::from_str(&txt).map_err(|e| format!("{} inválido: {e}", p.display()))?;
    let mut out = Vec::new();
    for e in v.as_array().unwrap_or(&vec![]) {
        let (Some(uuid), Some(nombre)) = (e["uuid"].as_str(), e["name"].as_str()) else {
            continue;
        };
        out.push(Entrada {
            uuid: uuid.to_string(),
            nombre: nombre.to_string(),
            level: e["level"].as_u64().map(|l| l as u8),
        });
    }
    Ok(out)
}

fn escribir_lista(dir: &std::path::Path, fichero: &str, lista: &[Entrada]) -> Result<(), String> {
    let v: Vec<Value> = lista
        .iter()
        .map(|e| {
            let mut o = json!({ "uuid": e.uuid, "name": e.nombre });
            if let Some(l) = e.level {
                o["level"] = json!(l);
                o["bypassesPlayerLimit"] = json!(false);
            }
            o
        })
        .collect();
    let p = dir.join(fichero);
    fs::write(&p, serde_json::to_string_pretty(&v).unwrap() + "\n")
        .map_err(|e| format!("escribir {}: {e}", p.display()))
}

/// `forge whitelist <dir> add|remove|list [nombres…]`
pub fn whitelist_cmd(args: &[String]) -> ExitCode {
    jugadores_cmd(args, "whitelist")
}

/// `forge op <dir> add|remove|list [nombres…]`
pub fn op_cmd(args: &[String]) -> ExitCode {
    jugadores_cmd(args, "op")
}

fn jugadores_cmd(args: &[String], modo: &str) -> ExitCode {
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if pos.len() < 2 {
        let uso = if modo == "whitelist" {
            "forge whitelist <dir> add|remove|list [jugadores…]"
        } else {
            "forge op <dir> add|remove|list [jugadores…]"
        };
        eprintln!("uso: {uso}");
        return ExitCode::from(2);
    }
    let dir = pos[0];
    let accion = pos[1].as_str();
    let nombres: Vec<&str> = pos[2..].iter().map(|s| s.as_str()).collect();

    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let raiz = resolver(dir);
    let (fichero, rcon_add, rcon_del) = if modo == "whitelist" {
        ("whitelist.json", "whitelist add", "whitelist remove")
    } else {
        ("ops.json", "op", "deop")
    };

    if accion == "list" {
        match leer_lista(&raiz, fichero) {
            Ok(l) if l.is_empty() => println!("{fichero}: vacío"),
            Ok(l) => {
                println!("{fichero} ({}):", l.len());
                for e in l {
                    match e.level {
                        Some(lvl) => println!("  {} — {} (nivel {lvl})", e.nombre, e.uuid),
                        None => println!("  {} — {}", e.nombre, e.uuid),
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    if nombres.is_empty() {
        eprintln!("di al menos un jugador para {accion}");
        return ExitCode::from(2);
    }

    let arriba = puerto_abierto(cfg.port) && cfg.rcon.enabled;
    let mut lista = match leer_lista(&raiz, fichero) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    for nombre in &nombres {
        let uuid = match uuid_para(&cfg, nombre) {
            Ok(u) => u,
            Err(e) => {
                eprintln!("✗ {nombre}: {e}");
                continue;
            }
        };
        match accion {
            "add" => {
                if lista.iter().any(|e| e.uuid == uuid) {
                    println!("• {nombre} ya estaba en {fichero}");
                } else {
                    lista.push(Entrada {
                        uuid: uuid.clone(),
                        nombre: nombre.to_string(),
                        level: (modo == "op").then_some(4),
                    });
                    println!("✓ {nombre} añadido a {fichero} ({uuid})");
                    if arriba {
                        di_al_server(&cfg, &format!("{rcon_add} {nombre}"));
                    }
                }
            }
            "remove" => {
                let antes = lista.len();
                lista.retain(|e| e.uuid != uuid);
                if lista.len() == antes {
                    println!("• {nombre} no estaba en {fichero}");
                } else {
                    println!("✓ {nombre} quitado de {fichero}");
                    if arriba {
                        di_al_server(&cfg, &format!("{rcon_del} {nombre}"));
                    }
                }
            }
            otro => {
                eprintln!("acción desconocida: {otro} (add|remove|list)");
                return ExitCode::from(2);
            }
        }
    }
    if let Err(e) = escribir_lista(&raiz, fichero, &lista) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    if accion == "add" && modo == "whitelist" && !cfg.whitelist {
        println!("⚠ la whitelist está desactivada en este server (enforce-whitelist=false):");
        println!("  actívala en server.properties o reinstala con --whitelist");
    }
    ExitCode::SUCCESS
}

/// manda un comando al server vivo, sin que un fallo tumbe el comando entero
fn di_al_server(cfg: &ServerConfig, cmd: &str) {
    if let Err(e) = with_rcon("127.0.0.1", cfg.rcon.port, &cfg.rcon.password, |r| {
        r.command(cmd)
    }) {
        eprintln!("  (RCON falló, el fichero quedó actualizado: {e})");
    }
}

/// espera a que el puerto se libere (tras un stop), hasta `seg` segundos
pub(crate) fn esperar_puerto_libre(puerto: u16, seg: u32) -> bool {
    for _ in 0..(seg * 4) {
        let abierto = TcpStream::connect_timeout(
            &format!("127.0.0.1:{puerto}").parse().unwrap(),
            Duration::from_millis(250),
        )
        .is_ok();
        if !abierto {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_offline_es_uuid_v3_estable() {
        let u = uuid_offline("Notch");
        assert_eq!(u, uuid_offline("Notch")); // determinista
        assert_ne!(u, uuid_offline("Otro"));
        let partes: Vec<&str> = u.split('-').collect();
        assert_eq!(
            partes.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(u.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        // versión 3 en el primer nibble del tercer grupo
        assert!(partes[2].starts_with('3'));
        // variante RFC 4122: tercer grupo de 4 empieza por 8,9,a,b
        assert!("89ab".contains(partes[3].chars().next().unwrap()));
    }

    #[test]
    fn ids_de_mojang_reciben_guiones() {
        assert_eq!(
            con_guiones("069a79f444e94726a5befca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
        // idem si ya traen guiones (robustez)
        assert_eq!(
            con_guiones("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
    }

    #[test]
    fn listas_roundtrip_en_disco() {
        let dir = std::env::temp_dir().join(format!("ob-players-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // ausente = vacío
        assert!(leer_lista(&dir, "whitelist.json").unwrap().is_empty());
        let lista = vec![Entrada {
            uuid: uuid_offline("Steve"),
            nombre: "Steve".into(),
            level: None,
        }];
        escribir_lista(&dir, "whitelist.json", &lista).unwrap();
        let leida = leer_lista(&dir, "whitelist.json").unwrap();
        assert_eq!(leida.len(), 1);
        assert_eq!(leida[0].nombre, "Steve");
        assert_eq!(leida[0].uuid, lista[0].uuid);
        // ops lleva level
        let ops = vec![Entrada {
            uuid: uuid_offline("Alex"),
            nombre: "Alex".into(),
            level: Some(4),
        }];
        escribir_lista(&dir, "ops.json", &ops).unwrap();
        assert_eq!(leer_lista(&dir, "ops.json").unwrap()[0].level, Some(4));
        fs::remove_dir_all(&dir).ok();
    }
}
