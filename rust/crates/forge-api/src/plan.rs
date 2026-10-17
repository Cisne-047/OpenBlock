//! Resolver del manifiesto de descargas, portado de `src/plan.ts`.
//!
//! El TS lo resuelve contra las APIs **antes** de generar el proyecto, para que
//! `apply` y los `install.sh`/`install.ps1` del zip usen las mismas URLs y los
//! mismos hashes. El modelo (`DownloadPlan`) vive en el núcleo porque los
//! generadores lo consumen; aquí está lo que habla con el mundo.
//!
//! Esto **no tiene golden y no puede tenerlo**: la salida lleva el número de build
//! y el sha256 que devuelve la API ese día. La verificación es la que DistroForge
//! usa para las APIs ajenas: ejecutar las dos implementaciones contra la misma API
//! y comparar los dos `plan.json`. Lo que sí es determinista —el orden de las
//! claves, los `dest`, los subdirectorios— está escrito para que ese `diff` salga
//! limpio a la primera.

use crate::engines::{self, PaperFamily};
use crate::modrinth;
use crate::mojang;
use crate::net;
use forge_core::catalog::{engine_by_id, perf_items_for};
use forge_core::modrinth::mrpack_server_files;
use forge_core::plan::{DownloadEntry, DownloadKind, DownloadPlan, Installer, LocalCopy};
use forge_core::types::{EngineId, ModpackKind, ServerConfig};

/// Error del resolver: el mismo mensaje que vería el usuario en el TS.
#[derive(Debug)]
pub enum Error {
    Red(net::Error),
    Motores(engines::Error),
    Modrinth(modrinth::Error),
    Mojang(mojang::Error),
    Mensaje(String),
}

impl From<net::Error> for Error {
    fn from(e: net::Error) -> Self {
        Self::Red(e)
    }
}

impl From<engines::Error> for Error {
    fn from(e: engines::Error) -> Self {
        Self::Motores(e)
    }
}

impl From<modrinth::Error> for Error {
    fn from(e: modrinth::Error) -> Self {
        Self::Modrinth(e)
    }
}

impl From<mojang::Error> for Error {
    fn from(e: mojang::Error) -> Self {
        Self::Mojang(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Red(e) => write!(f, "{e}"),
            Self::Motores(e) => write!(f, "{e}"),
            Self::Modrinth(e) => write!(f, "{e}"),
            Self::Mojang(e) => write!(f, "{e}"),
            Self::Mensaje(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

/// El loader tag de Modrinth para pedir versiones de plugins/mods (`modrinthLoader`).
pub fn modrinth_loader(engine: EngineId) -> Option<&'static str> {
    match engine {
        EngineId::Paper | EngineId::Purpur => Some("paper"),
        EngineId::Folia => Some("folia"),
        EngineId::Fabric => Some("fabric"),
        EngineId::Quilt => Some("quilt"),
        EngineId::Forge => Some("forge"),
        EngineId::Neoforge => Some("neoforge"),
        EngineId::Vanilla => None,
    }
}

/// Un aviso del resolver: el TS lo escribe por consola y sigue. Aquí se devuelve
/// para que la CLI decida dónde imprimirlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso(pub String);

/// Resuelve el plan completo (`buildDownloadPlan`).
///
/// Devuelve también los avisos: en el TS un plugin sin build para esa versión no
/// tumba el plan, solo se salta con un `console.warn`.
pub fn build_download_plan(cfg: &ServerConfig) -> Result<(DownloadPlan, Vec<Aviso>), Error> {
    let mut entries: Vec<DownloadEntry> = Vec::new();
    let mut local_copies: Vec<LocalCopy> = Vec::new();
    let mut avisos: Vec<Aviso> = Vec::new();

    let engine = engine_by_id(cfg.engine);

    // El TS construye un objeto parcial y lo extiende con spread, así que el orden
    // de las claves del JSON es el de inserción: engineFilename, engineResolved,
    // [loaderResolved | installer], [unresolvedCf], entries, localCopies. El struct
    // de Rust declara los campos en ese mismo orden.
    // El `match` de abajo es exhaustivo y asigna las dos: no hace falta valor
    // inicial, y con él el compilador avisa de una asignación muerta.
    let engine_filename: String;
    let mut engine_resolved: String;
    let mut loader_resolved: Option<String> = None;
    let mut installer: Option<Installer> = None;
    let mut unresolved_cf: Option<i32> = None;

    match cfg.engine {
        EngineId::Vanilla => {
            let dl = mojang::vanilla_server_for(&cfg.mc_version)?;
            engine_filename = "server.jar".to_string();
            engine_resolved = cfg.mc_version.clone();
            entries.push(DownloadEntry {
                kind: DownloadKind::Engine,
                label: format!("Vanilla {}", cfg.mc_version),
                url: dl.url,
                dest: "server.jar".to_string(),
                sha256: None,
                sha1: None,
                sha512: None,
            });
        }
        EngineId::Paper | EngineId::Purpur | EngineId::Folia => {
            let family =
                PaperFamily::parse(engine.id.as_str()).expect("la familia es la del catálogo");
            let dl = engines::paper_family_download(family, &cfg.mc_version, cfg.build.as_deref())?;
            engine_filename = dl.filename.clone();
            engine_resolved = dl.resolved.clone();
            entries.push(DownloadEntry {
                kind: DownloadKind::Engine,
                label: format!("{} {} ({})", engine.label, cfg.mc_version, dl.resolved),
                url: dl.url,
                dest: dl.filename,
                sha256: dl.sha256,
                sha1: None,
                sha512: None,
            });
        }
        EngineId::Fabric | EngineId::Quilt => {
            let dl = if cfg.engine == EngineId::Fabric {
                engines::fabric_download(
                    &cfg.mc_version,
                    cfg.loader_build.as_deref(),
                    cfg.installer_build.as_deref(),
                )?
            } else {
                engines::quilt_download(
                    &cfg.mc_version,
                    cfg.loader_build.as_deref(),
                    cfg.installer_build.as_deref(),
                )?
            };
            engine_filename = dl.filename.clone();
            engine_resolved = dl.resolved.clone();
            loader_resolved = Some(dl.resolved.clone());
            entries.push(DownloadEntry {
                kind: DownloadKind::Engine,
                label: format!("{} {} ({})", engine.label, cfg.mc_version, dl.resolved),
                url: dl.url,
                dest: dl.filename,
                sha256: None,
                sha1: None,
                sha512: None,
            });
        }
        EngineId::Forge | EngineId::Neoforge => {
            let dl = if cfg.engine == EngineId::Forge {
                engines::forge_download(&cfg.mc_version, cfg.build.as_deref())?
            } else {
                engines::neoforge_download(&cfg.mc_version, cfg.build.as_deref())?
            };
            engine_filename = dl.filename.clone();
            engine_resolved = dl.resolved.clone();
            installer = Some(Installer {
                jar: dl.filename.clone(),
                engine: cfg.engine.as_str().to_string(),
            });
            entries.push(DownloadEntry {
                kind: DownloadKind::Engine,
                label: format!("{} {} ({})", engine.label, cfg.mc_version, dl.resolved),
                url: dl.url,
                dest: dl.filename,
                sha256: None,
                sha1: None,
                sha512: None,
            });
        }
    }

    let mr_loader = modrinth_loader(cfg.engine);
    let mc_filter = vec![cfg.mc_version.clone()];

    // plugins/mods del catálogo de rendimiento (chunkyborder solo si se pide)
    if let Some(loader) = mr_loader {
        for item in perf_items_for(engine.loader, &cfg.mc_version) {
            if !item.default && !(item.id == "chunkyborder" && cfg.chunkyborder) {
                continue;
            }
            match modrinth::latest_version(item.slug, Some(&mc_filter), Some(&[loader.to_string()]))
            {
                Ok(v) => {
                    let Some(f) = modrinth::primary_file(&v) else {
                        continue;
                    };
                    let subdir = if item.kind == "plugin" {
                        "plugins"
                    } else {
                        "mods"
                    };
                    entries.push(DownloadEntry {
                        kind: if item.kind == "plugin" {
                            DownloadKind::Plugin
                        } else {
                            DownloadKind::Mod
                        },
                        label: item.label.to_string(),
                        url: f.url.clone(),
                        dest: format!("{subdir}/{}", f.filename),
                        sha256: None,
                        sha1: None,
                        sha512: f.hashes.as_ref().and_then(|h| h.sha512.clone()),
                    });
                }
                Err(e) => {
                    // item sin build para esa versión: se salta con aviso en el plan
                    unresolved_cf.get_or_insert(0);
                    avisos.push(Aviso(format!(
                        "[plan] {}: sin build para {} ({e})",
                        item.label, cfg.mc_version
                    )));
                }
            }
        }
    }

    // plugins/mods propios del usuario (URLs directas; los locales los copia apply)
    for item in &cfg.custom_items {
        let subdir = if item.kind == "plugin" {
            "plugins"
        } else {
            "mods"
        };
        let nombre = if item.name.ends_with(".jar") {
            item.name.clone()
        } else {
            format!("{}.jar", item.name)
        };
        if let Some(url) = &item.url {
            entries.push(DownloadEntry {
                kind: if item.kind == "plugin" {
                    DownloadKind::Plugin
                } else {
                    DownloadKind::Mod
                },
                label: format!("{} (custom)", item.name),
                url: url.clone(),
                dest: format!("{subdir}/{nombre}"),
                sha256: None,
                sha1: None,
                sha512: None,
            });
        } else if let Some(path) = &item.path {
            local_copies.push(LocalCopy {
                src: path.clone(),
                dest: format!("{subdir}/{nombre}"),
            });
        }
    }

    // modpacks
    match cfg.modpack.kind {
        ModpackKind::Modrinth => {
            let index = modrinth::mrpack_index(&cfg.modpack.url)?;
            engine_resolved += &format!(" + modpack {} {}", index.name, index.version_id);
            for f in mrpack_server_files(&index) {
                entries.push(DownloadEntry {
                    kind: DownloadKind::ModpackFile,
                    label: f.path.clone(),
                    url: f.downloads.first().cloned().unwrap_or_default(),
                    dest: f.path.clone(),
                    sha256: None,
                    sha1: None,
                    sha512: Some(f.hashes.sha512.clone().unwrap_or_default()),
                });
            }
        }
        ModpackKind::Curseforge => {
            // los overrides/mods van dentro del zip que baja el usuario; aquí solo
            // registramos la intención: el manifest se resuelve con OPENBLOCK_CF_KEY
            unresolved_cf = Some(-1);
        }
        ModpackKind::None => {}
    }

    Ok((
        DownloadPlan {
            engine_filename,
            engine_resolved,
            loader_resolved,
            installer,
            unresolved_cf,
            entries,
            local_copies,
        },
        avisos,
    ))
}

/// Resuelve las URLs de un manifest CurseForge (`resolveCurseforgeEntries`).
///
/// El `.mrpack` de CurseForge que exporta el launcher lleva los ficheros dentro;
/// aquí solo se piden las URLs de los que son `required`, y sin key la API no las
/// da, que es exactamente lo que dice el mensaje del TS.
pub fn resolve_curseforge_entries(
    manifest_json: &str,
    key: Option<&str>,
) -> Result<Vec<DownloadEntry>, Error> {
    let manifest = forge_core::modrinth::parse_cf_manifest(manifest_json)
        .map_err(|e| Error::Mensaje(e.to_string()))?;
    let mut out = Vec::new();
    for f in &manifest.files {
        if !f.required {
            continue;
        }
        out.push(DownloadEntry {
            kind: DownloadKind::ModpackFile,
            label: format!("CF {}/{}", f.project_id, f.file_id),
            url: modrinth::curseforge_file_url(key, f.project_id, f.file_id)?,
            // el nombre real lo dice la API de descarga
            dest: String::new(),
            sha256: None,
            sha1: None,
            sha512: None,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::modrinth_loader;
    use forge_core::types::EngineId;

    #[test]
    fn el_loader_de_modrinth_por_engine() {
        assert_eq!(modrinth_loader(EngineId::Paper), Some("paper"));
        assert_eq!(modrinth_loader(EngineId::Purpur), Some("paper"));
        assert_eq!(modrinth_loader(EngineId::Folia), Some("folia"));
        assert_eq!(modrinth_loader(EngineId::Fabric), Some("fabric"));
        assert_eq!(modrinth_loader(EngineId::Quilt), Some("quilt"));
        assert_eq!(modrinth_loader(EngineId::Forge), Some("forge"));
        assert_eq!(modrinth_loader(EngineId::Neoforge), Some("neoforge"));
        // vanilla no tiene loader: no se le piden plugins a Modrinth
        assert_eq!(modrinth_loader(EngineId::Vanilla), None);
    }
}
