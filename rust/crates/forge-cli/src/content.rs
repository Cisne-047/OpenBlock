//! `forge search` / `forge plugins` / `forge mods` — elegir e instalar contenido.
//!
//! `search` consulta Modrinth (plugins o mods según `--mod`); `plugins add
//! <slug|url>` resuelve la versión para TU versión de MC y loader, baja el jar
//! a `plugins/` (o `mods/`) y avisa de que hace falta reiniciar —sin PlugMan no
//! hay hot-reload—. `update` re-resuelve lo que OpenBlock sabe resolver: el
//! catálogo de rendimiento y los custom con URL. `list` y `remove` son lo que
//! suenan.

use crate::status::cargar_cfg;
use crate::{resolver, ExitCode};
use forge_core::EngineId;
use std::fs;
use std::path::Path;

/// `forge search <texto> [--mod] [--limit N]`
pub fn search_cmd(args: &[String]) -> ExitCode {
    let mut query: Option<&String> = None;
    let mut tipo = "plugin";
    let mut limit = 10usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--mod" => {
                tipo = "mod";
                i += 1;
            }
            "--limit" if i + 1 < args.len() => {
                limit = args[i + 1].parse().unwrap_or(10);
                i += 2;
            }
            otro if !otro.starts_with('-') && query.is_none() => {
                query = Some(&args[i]);
                i += 1;
            }
            _ => i += 1,
        }
    }
    let Some(query) = query else {
        eprintln!("uso: forge search <texto> [--mod] [--limit N]");
        return ExitCode::from(2);
    };
    match forge_api::modrinth::search(query, tipo, limit) {
        Ok(hits) if hits.is_empty() => {
            println!("Sin resultados para «{query}» ({tipo}s)");
            ExitCode::SUCCESS
        }
        Ok(hits) => {
            println!(
                "Resultados de Modrinth ({tipo}s) — instala con `forge {}s add <slug>`:",
                tipo
            );
            for h in &hits {
                println!(
                    "  {:<24} {:>9} descargas  {}",
                    h.slug,
                    formato_descargas(h.downloads),
                    recorte(&h.description, 60)
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Búsqueda fallida: {e}");
            ExitCode::FAILURE
        }
    }
}

fn formato_descargas(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        format!("{n}")
    }
}

fn recorte(s: &str, max: usize) -> String {
    let limpio = s.replace(['\n', '\r'], " ");
    if limpio.chars().count() > max {
        let c: String = limpio.chars().take(max).collect();
        format!("{c}…")
    } else {
        limpio
    }
}

/// `forge plugins <dir> add|remove|list|update […]` (mods: igual con `mods`)
pub fn plugins_cmd(args: &[String]) -> ExitCode {
    contenido_cmd(args, false)
}

pub fn mods_cmd(args: &[String]) -> ExitCode {
    contenido_cmd(args, true)
}

fn contenido_cmd(args: &[String], es_mods: bool) -> ExitCode {
    let comando = if es_mods { "mods" } else { "plugins" };
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if pos.len() < 2 {
        eprintln!("uso: forge {comando} <dir> add|remove|list|update [slug|url|nombre…]");
        eprintln!("  add acepta slug de Modrinth o URL directa al .jar");
        return ExitCode::from(2);
    }
    let (dir, accion) = (pos[0], pos[1].as_str());
    let valores: Vec<&str> = pos[2..].iter().map(|s| s.as_str()).collect();
    let (cfg, _) = match cargar_cfg(dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let raiz = resolver(dir);
    let carpeta = if es_mods { "mods" } else { "plugins" };
    let destino_dir = raiz.join(carpeta);
    fs::create_dir_all(&destino_dir).ok();

    match accion {
        "list" => {
            let jars = listar_jars(&destino_dir);
            if jars.is_empty() {
                println!("{carpeta}/: vacío");
            } else {
                println!("{carpeta}/ ({}):", jars.len());
                for j in jars {
                    println!("  {j}");
                }
            }
            ExitCode::SUCCESS
        }
        "add" => {
            if valores.is_empty() {
                eprintln!("dime al menos un slug o URL (`forge search …` para encontrarlos)");
                return ExitCode::from(2);
            }
            let mut fallos = 0;
            for v in &valores {
                if let Err(e) = instalar(&cfg, v, &destino_dir, es_mods) {
                    eprintln!("✗ {v}: {e}");
                    fallos += 1;
                }
            }
            if fallos > 0 {
                return ExitCode::FAILURE;
            }
            println!("Hecho. Hace falta reiniciar el server para que cargue (sin PlugMan no hay hot-reload).");
            ExitCode::SUCCESS
        }
        "remove" => {
            if valores.is_empty() {
                eprintln!("dime al menos un nombre (`forge {comando} <dir> list` los muestra)");
                return ExitCode::from(2);
            }
            for v in &valores {
                let coincide = listar_jars(&destino_dir)
                    .into_iter()
                    .find(|j| j.to_lowercase().contains(&v.to_lowercase()));
                match coincide {
                    Some(j) => match fs::remove_file(destino_dir.join(&j)) {
                        Ok(()) => println!("✓ {j} fuera (reinicia para que deje de cargar)"),
                        Err(e) => eprintln!("✗ borrar {j}: {e}"),
                    },
                    None => println!("• nada en {carpeta}/ parecido a «{v}»"),
                }
            }
            ExitCode::SUCCESS
        }
        "update" => {
            let (actualizados, fallos) = actualizar_catalogo(&cfg, &raiz);
            if actualizados.is_empty() && fallos == 0 {
                println!("Nada que actualizar: el catálogo está al día.");
            }
            if fallos > 0 {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        otra => {
            eprintln!("acción desconocida: {otra} (add|remove|list|update)");
            ExitCode::from(2)
        }
    }
}

fn listar_jars(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jar"))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    out.sort();
    out
}

/// baja el jar (slug Modrinth con la versión para TU mc/loader, o URL directa)
fn instalar(
    cfg: &forge_core::ServerConfig,
    que: &str,
    destino_dir: &Path,
    es_mods: bool,
) -> Result<(), String> {
    let (url, nombre, sha512) = if que.starts_with("http") {
        let nombre = que.rsplit('/').next().unwrap_or("custom.jar").to_string();
        (que.to_string(), nombre, None)
    } else {
        let loaders: Vec<String> = vec![loader_de(cfg.engine).to_string()];
        let mcs: Vec<String> = vec![cfg.mc_version.clone()];
        let v = forge_api::modrinth::latest_version(que, Some(&mcs), Some(&loaders))
            .map_err(|e| e.to_string())?;
        let f = forge_core::modrinth::primary_file(&v)
            .ok_or_else(|| format!("{que} sin fichero primario"))?;
        (
            f.url.clone(),
            f.filename.clone(),
            f.hashes.as_ref().and_then(|h| h.sha512.clone()),
        )
    };
    let dest = destino_dir.join(&nombre);
    println!("↓ {nombre} …");
    let opts = forge_api::net::OpcionesDescarga::default();
    forge_api::net::download_to(&url, &dest, &opts).map_err(|e| e.to_string())?;
    if let Some(h) = sha512 {
        if let Some(err) = verificar_sha512(&dest, &h) {
            let _ = fs::remove_file(&dest);
            return Err(err);
        }
    }
    println!("✓ {nombre} en {}", destino_dir.display());
    let _ = es_mods;
    Ok(())
}

fn loader_de(engine: EngineId) -> &'static str {
    match engine {
        EngineId::Paper | EngineId::Purpur => "paper",
        EngineId::Folia => "folia",
        EngineId::Fabric => "fabric",
        EngineId::Quilt => "quilt",
        EngineId::Forge => "forge",
        EngineId::Neoforge => "neoforge",
        EngineId::Vanilla => "paper", // vanilla no tiene plugins: paper como límite inferior
    }
}

fn verificar_sha512(dest: &Path, esperado: &str) -> Option<String> {
    use sha2::{Digest, Sha512};
    let bytes = fs::read(dest).ok()?;
    let hago = Sha512::digest(&bytes);
    let hex: String = hago.iter().map(|b| format!("{b:02x}")).collect();
    if hex != esperado.to_lowercase() {
        Some(format!("sha512 no coincide (esperado {esperado})"))
    } else {
        None
    }
}

/// re-resuelve catálogo + custom con URL contra Modrinth y baja los cambios
fn actualizar_catalogo(cfg: &forge_core::ServerConfig, raiz: &Path) -> (Vec<String>, usize) {
    let mut actualizados = Vec::new();
    let mut fallos = 0usize;
    let loaders: Vec<String> = vec![loader_de(cfg.engine).to_string()];
    let mcs: Vec<String> = vec![cfg.mc_version.clone()];
    let items = forge_core::catalog::perf_items_for(tipo_loader(cfg.engine), &cfg.mc_version);
    for it in items.iter().filter(|i| i.default) {
        let Ok(v) = forge_api::modrinth::latest_version(it.slug, Some(&mcs), Some(&loaders)) else {
            continue; // sin build para esta versión: el plan ya lo avisó
        };
        let Some(f) = forge_core::modrinth::primary_file(&v) else {
            continue;
        };
        let carpeta = if it.kind == "plugin" {
            "plugins"
        } else {
            "mods"
        };
        let dest = raiz.join(carpeta);
        fs::create_dir_all(&dest).ok();
        let destino = dest.join(&f.filename);
        if destino.exists() {
            continue; // ya está
        }
        // borra la versión vieja del mismo proyecto si se puede identificar por slug
        for viejo in listar_jars(&dest) {
            if viejo
                .to_lowercase()
                .contains(&it.slug.replace('-', "").to_lowercase())
            {
                let _ = fs::remove_file(dest.join(&viejo));
            }
        }
        println!("↓ {} (nuevo: {}) …", it.label, f.filename);
        let opts = forge_api::net::OpcionesDescarga::default();
        match forge_api::net::download_to(&f.url, &destino, &opts) {
            Ok(()) => {
                println!("✓ {} actualizado a {}", it.label, v.version_number);
                actualizados.push(it.label.to_string());
            }
            Err(e) => {
                eprintln!("✗ {}: {e}", it.label);
                fallos += 1;
            }
        }
    }
    // customs con URL: re-descarga tal cual (la URL ya apunta a la versión buena)
    for c in cfg
        .custom_items
        .iter()
        .filter(|c| c.url.as_deref().is_some_and(|u| !u.is_empty()))
    {
        let carpeta2 = if c.kind == "mod" { "mods" } else { "plugins" };
        let d = raiz.join(carpeta2);
        fs::create_dir_all(&d).ok();
        let destino = d.join(format!("{}.jar", c.name));
        println!("↓ {} (custom) …", c.name);
        let opts = forge_api::net::OpcionesDescarga::default();
        match forge_api::net::download_to(c.url.as_deref().unwrap_or(""), &destino, &opts) {
            Ok(()) => {
                println!("✓ {} re-descargado", c.name);
                actualizados.push(c.name.clone());
            }
            Err(e) => {
                eprintln!("✗ {}: {e}", c.name);
                fallos += 1;
            }
        }
    }
    (actualizados, fallos)
}

fn tipo_loader(engine: EngineId) -> forge_core::LoaderKind {
    match engine {
        EngineId::Paper | EngineId::Purpur => forge_core::LoaderKind::Paper,
        EngineId::Folia => forge_core::LoaderKind::Paper,
        EngineId::Fabric => forge_core::LoaderKind::Fabric,
        EngineId::Quilt => forge_core::LoaderKind::Quilt,
        EngineId::Forge => forge_core::LoaderKind::Forge,
        EngineId::Neoforge => forge_core::LoaderKind::Neoforge,
        EngineId::Vanilla => forge_core::LoaderKind::None,
    }
}

#[cfg(test)]
mod tests {
    use super::{formato_descargas, recorte};

    #[test]
    fn presentacion_de_resultados() {
        assert_eq!(formato_descargas(1_234_567), "1.2M");
        assert_eq!(formato_descargas(45_600), "45k");
        assert_eq!(formato_descargas(999), "999");
        assert_eq!(recorte("corto", 10), "corto");
        let largo = "x".repeat(80);
        let r = recorte(&largo, 60);
        assert!(r.ends_with('…') && r.chars().count() == 61);
    }
}
