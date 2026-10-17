//! Resolución de descargas por engine, portado de `src/api/engines.ts`.
//!
//! Cada función de red tiene su parte pura al lado (`*_from`) y el golden compara
//! **esa**: la respuesta de una API ajena cambia sin avisar, el mapeo no.
//!
//! Las rarezas del TS se reproducen enteras y están comentadas una a una. No son
//! hipótesis: son lo que hace el código que hay publicado.

use crate::js;
use crate::net;
use crate::url;
use serde::Deserialize;
use serde_json::{Number, Value};

/// El jar resuelto de un engine. El orden de los campos es el del TS, y los
/// opcionales **se omiten** en el JSON, que es lo que hace `JSON.stringify` con
/// un `undefined`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EngineDownload {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub filename: String,
    /// forge/neoforge: hay que ejecutar el installer antes del arranque
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<bool>,
    pub resolved: String,
}

/// Una build o versión ofrecida al usuario.
///
/// `id` es un `Value` a propósito: el TS lo deja crudo en algunas rutas (`{ id }`)
/// y lo pasa por `String()` en otras, y ahí está la diferencia entre `5` y `"5"`.
/// Con `Option` se cubre además el campo que no está, que en `JSON.stringify`
/// desaparece.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BuildInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

impl BuildInfo {
    /// El id como texto, para la CLI. Un id ausente es `""`, no un panic.
    pub fn id_str(&self) -> String {
        js::string(self.id.as_ref())
    }
}

/// Todo lo que puede salir mal. `Mensaje` es literalmente el mensaje del TS:
/// así el error que ve el usuario es el mismo en los dos lados.
#[derive(Debug)]
pub enum Error {
    Red(net::Error),
    Mensaje(String),
}

impl From<net::Error> for Error {
    fn from(e: net::Error) -> Self {
        Self::Red(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Red(e) => write!(f, "{e}"),
            Self::Mensaje(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

/// La familia de PaperMC: comparten API pero no endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperFamily {
    Paper,
    Purpur,
    Folia,
}

impl PaperFamily {
    /// El nombre tal como lo usa el catálogo del TS.
    pub fn como_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Purpur => "purpur",
            Self::Folia => "folia",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "paper" => Some(Self::Paper),
            "purpur" => Some(Self::Purpur),
            "folia" => Some(Self::Folia),
            _ => None,
        }
    }
}

// --- familia PaperMC (fill v3 con fallback a v2) ---

/// `Record<string, PaperDownloadRaw>` **conservando el orden del documento**.
///
/// `Object.values(downloads)[0]` devuelve el primero que se escribió en el JSON.
/// Un `BTreeMap` de serde_json los ordena por clave y daría otro; y activar
/// `preserve_order` cambiaría el orden de todos los mapas del proyecto. Un
/// visitante propio es lo único que respeta el documento sin tocar el resto.
#[derive(Debug, Clone, Default)]
pub struct Descargas(Vec<(String, PaperDownloadRaw)>);

impl Descargas {
    fn get(&self, clave: &str) -> Option<&PaperDownloadRaw> {
        self.0.iter().find(|(k, _)| k == clave).map(|(_, v)| v)
    }

    fn primero(&self) -> Option<&PaperDownloadRaw> {
        self.0.first().map(|(_, v)| v)
    }
}

impl<'de> Deserialize<'de> for Descargas {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitante;

        impl<'de> serde::de::Visitor<'de> for Visitante {
            type Value = Descargas;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("un objeto de descargas")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<Self::Value, A::Error> {
                let mut pares = Vec::new();
                while let Some((k, v)) = m.next_entry::<String, PaperDownloadRaw>()? {
                    pares.push((k, v));
                }
                Ok(Descargas(pares))
            }
        }

        d.deserialize_map(Visitante)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperDownloadRaw {
    pub url: String,
    pub sha256: Option<String>,
    pub name: Option<String>,
}

/// Un elemento de la lista de builds de fill v3. Todo opcional: el TS lee los
/// campos sin comprobarlos, y un elemento raro solo hace que la comparación del
/// `reduce` salga `false`, no que reviente.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PaperBuildFullRaw {
    pub id: Option<Number>,
    pub downloads: Option<Descargas>,
}

/// lista de builds disponibles de paper/folia para una versión de MC
pub fn paper_builds(project: &str, mc: &str) -> Vec<BuildInfo> {
    let url = format!(
        "https://fill.papermc.io/v3/projects/{project}/versions/{}/builds",
        url::encode_uri_component(mc)
    );
    match net::get_json::<Value>(&url) {
        Ok(raw) => paper_builds_from(&raw),
        Err(_) => Vec::new(),
    }
}

/// el mapeo de `paperBuilds`: nueva primero, y a la mínima se queda en vacío
pub fn paper_builds_from(raw: &Value) -> Vec<BuildInfo> {
    let Some(lista) = raw.as_array() else {
        // en el TS esto es un TypeError dentro del `try` de `paperBuilds`
        return Vec::new();
    };
    let mut pares: Vec<(f64, BuildInfo)> = lista
        .iter()
        .map(|b| {
            let id = b.get("id");
            (
                js::number(id),
                BuildInfo {
                    id: Some(Value::String(js::string(id))),
                    channel: b.get("channel").and_then(Value::as_str).map(str::to_string),
                },
            )
        })
        .collect();
    // `Number(b.id) - Number(a.id)`: descendente por número, no por texto, y un
    // comparador que devuelve NaN no toca el orden
    pares.sort_by(|a, b| match (b.0 - a.0).partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Less) => std::cmp::Ordering::Less,
        Some(std::cmp::Ordering::Greater) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });
    pares.into_iter().map(|(_, info)| info).collect()
}

fn papermc_v3(project: &str, mc: &str, build: Option<&str>) -> Option<EngineDownload> {
    let mc_enc = url::encode_uri_component(mc);
    let destino = match build {
        Some(b) => format!(
            "https://fill.papermc.io/v3/projects/{project}/versions/{mc_enc}/builds/{}",
            url::encode_uri_component(b)
        ),
        None => format!(
            "https://fill.papermc.io/v3/projects/{project}/versions/{mc_enc}/builds?channel=STABLE"
        ),
    };
    let raw = net::get_json::<Value>(&destino).ok()?;
    papermc_v3_from(&raw, project, mc)
}

/// el mapeo de `papermcV3`.
///
/// Ojo con el `reduce`: **no** es el máximo. El acumulador vuelve a `builds[0]` en
/// cuanto una build no supera a la anterior, así que con los ids `[5, 6, 7, 6]`
/// "la última" que sale es la 5. Se reproduce tal cual: el día que la API devuelva
/// ese orden, el puerto tiene que equivocarse igual.
///
/// Y el `??` de las descargas es por orden de escritura: `server:default` si está,
/// y si no la primera del documento (no la primera por orden alfabético).
pub fn papermc_v3_from(raw: &Value, project: &str, mc: &str) -> Option<EngineDownload> {
    // build exacta devuelve objeto único; lista devuelve array
    let builds: Vec<PaperBuildFullRaw> = match raw.as_array() {
        Some(lista) => lista.iter().map(elemento_de_build).collect(),
        None => vec![elemento_de_build(raw)],
    };
    if builds.is_empty() {
        return None;
    }
    let mut acc = &builds[0];
    for b in &builds {
        let mayor = match (b.id.as_ref(), acc.id.as_ref()) {
            // `undefined > n` y `NaN > n` son false en JS, y aquí también
            (Some(x), Some(y)) => x.as_f64().unwrap_or(f64::NAN) > y.as_f64().unwrap_or(f64::NAN),
            _ => false,
        };
        acc = if mayor { b } else { &builds[0] };
    }
    let latest = acc;
    let descargas = latest.downloads.as_ref()?;
    let dl = descargas
        .get("server:default")
        .or_else(|| descargas.primero())?;
    Some(EngineDownload {
        url: dl.url.clone(),
        sha256: dl.sha256.clone(),
        filename: format!("{project}-{mc}-{}.jar", id_de(latest)),
        installer: None,
        resolved: format!("build {}", id_de(latest)),
    })
}

/// Un elemento que no se puede leer como build se queda vacío, como cuando el TS
/// lee una propiedad que no existe.
fn elemento_de_build(v: &Value) -> PaperBuildFullRaw {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

fn id_de(b: &PaperBuildFullRaw) -> String {
    b.id.as_ref()
        .map(js::num)
        .unwrap_or_else(|| "undefined".to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperV2Raw {
    pub builds: Vec<PaperV2BuildRaw>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperV2BuildRaw {
    pub build: Number,
    pub downloads: PaperV2Downloads,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperV2Downloads {
    pub application: PaperV2Application,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperV2Application {
    pub url: String,
    pub name: String,
    pub sha256: Option<String>,
}

fn papermc_v2(project: &str, mc: &str, build: Option<&str>) -> Result<EngineDownload, Error> {
    let destino = format!(
        "https://api.papermc.io/v2/projects/{project}/versions/{}/builds",
        url::encode_uri_component(mc)
    );
    let data: Value = net::get_json(&destino)?;
    papermc_v2_from(&data, project, mc, build)
}

/// el mapeo de `papermcV2`: aquí sí se busca la build pedida, y si no está, lanza
pub fn papermc_v2_from(
    data: &Value,
    project: &str,
    mc: &str,
    build: Option<&str>,
) -> Result<EngineDownload, Error> {
    let data: PaperV2Raw = serde_json::from_value(data.clone())
        .map_err(|e| Error::Mensaje(format!("respuesta de {project} inesperada: {e}")))?;
    let chosen = match build {
        Some(b) => data.builds.iter().find(|x| js::num(&x.build) == b),
        None => data.builds.last(),
    };
    // el mensaje del TS interpola `build`, que sin build pedida es `undefined`
    let elegida = chosen.ok_or_else(|| {
        Error::Mensaje(format!(
            "Sin build {} de {project} para {mc}",
            build.unwrap_or("undefined")
        ))
    })?;
    Ok(EngineDownload {
        url: elegida.downloads.application.url.clone(),
        sha256: elegida.downloads.application.sha256.clone(),
        filename: elegida.downloads.application.name.clone(),
        installer: None,
        resolved: format!("build {}", js::num(&elegida.build)),
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurpurBuildsRaw {
    pub builds: Option<PurpurBuildsAll>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurpurBuildsAll {
    pub all: Option<Vec<Value>>,
}

/// builds de Purpur para una versión de MC (nueva primero)
pub fn purpur_builds(mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let destino = format!(
        "https://api.purpurmc.org/v2/purpur/{}",
        url::encode_uri_component(mc)
    );
    let data: Value = net::get_json(&destino)?;
    purpur_builds_from(&data)
}

pub fn purpur_builds_from(data: &Value) -> Result<Vec<BuildInfo>, Error> {
    let data: PurpurBuildsRaw = serde_json::from_value(data.clone())
        .map_err(|e| Error::Mensaje(format!("respuesta de purpur inesperada: {e}")))?;
    let mut all = data.builds.and_then(|b| b.all).unwrap_or_default();
    all.reverse();
    Ok(all
        .into_iter()
        .map(|id| BuildInfo {
            id: Some(id),
            channel: None,
        })
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurpurInfoRaw {
    pub build: PurpurInfoBuild,
    pub builds: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurpurInfoBuild {
    pub build: String,
    pub download: String,
}

pub fn purpur_download_from(
    info: &Value,
    mc: &str,
    build: Option<&str>,
) -> Result<EngineDownload, Error> {
    let info: PurpurInfoRaw = serde_json::from_value(info.clone())
        .map_err(|e| Error::Mensaje(format!("respuesta de purpur inesperada: {e}")))?;
    let chosen = build
        .map(str::to_string)
        .unwrap_or_else(|| info.build.build.clone());
    Ok(EngineDownload {
        url: format!(
            "https://api.purpurmc.org/v2/purpur/{}/{}/download",
            url::encode_uri_component(mc),
            url::encode_uri_component(&chosen)
        ),
        sha256: None,
        filename: format!("purpur-{mc}-{chosen}.jar"),
        installer: None,
        resolved: format!("build {chosen}"),
    })
}

pub fn paper_family_download(
    engine: PaperFamily,
    mc: &str,
    build: Option<&str>,
) -> Result<EngineDownload, Error> {
    if engine == PaperFamily::Purpur {
        let destino = format!(
            "https://api.purpurmc.org/v2/purpur/{}",
            url::encode_uri_component(mc)
        );
        let info: Value = net::get_json(&destino)?;
        return purpur_download_from(&info, mc, build);
    }
    let project = engine.como_str();
    if let Some(v3) = papermc_v3(project, mc, build) {
        return Ok(v3);
    }
    papermc_v2(project, mc, build)
}

// --- Fabric / Quilt (meta APIs) ---

fn latest_version(list_url: &str, field: &str) -> Result<String, Error> {
    let list: Value = net::get_json(list_url)?;
    latest_version_from(&list, field, list_url)
}

/// el `find` de las meta APIs.
///
/// El filtro es `stable !== false`, así que una entrada **sin** campo `stable`
/// cuenta como estable: gana la primera de la lista, no la primera estable de
/// verdad. Y `String(first[field])` con el campo ausente es la cadena
/// `"undefined"`, que acabaría dentro de una URL de descarga.
pub fn latest_version_from(list: &Value, field: &str, list_url: &str) -> Result<String, Error> {
    let vacia = || Error::Mensaje(format!("Meta API vacía: {list_url}"));
    let arr = list.as_array().ok_or_else(vacia)?;
    let first = arr
        .iter()
        // `stable !== false`: una entrada sin el campo también pasa
        .find(|e| e.get("stable") != Some(&Value::Bool(false)))
        .or_else(|| arr.first())
        .ok_or_else(vacia)?;
    Ok(js::string(first.get(field)))
}

#[derive(Debug, Clone, Deserialize)]
pub struct FabricLoaderRaw {
    pub loader: Option<FabricLoaderVersion>,
    pub version: Option<Value>,
    pub stable: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FabricLoaderVersion {
    pub version: Option<Value>,
}

/// versiones de loader de Fabric disponibles para una versión de MC (nueva primero)
pub fn fabric_loader_versions(mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let destino = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}",
        url::encode_uri_component(mc)
    );
    let list: Value = net::get_json(&destino)?;
    fabric_loader_versions_from(&list)
}

pub fn fabric_loader_versions_from(list: &Value) -> Result<Vec<BuildInfo>, Error> {
    let list = lista_de::<FabricLoaderRaw>(list, "fabric")?;
    Ok(list
        .into_iter()
        .map(|e| BuildInfo {
            // `e.loader?.version ?? String(e.version)`: el `??` también cae con null
            id: Some(
                e.loader
                    .and_then(|l| l.version)
                    .filter(|v| !v.is_null())
                    .unwrap_or_else(|| Value::String(js::string(e.version.as_ref()))),
            ),
            channel: Some(canal(e.stable.as_ref()).to_string()),
        })
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
pub struct FabricInstallerRaw {
    pub version: Option<Value>,
    pub stable: Option<Value>,
}

pub fn fabric_installer_versions() -> Result<Vec<BuildInfo>, Error> {
    let list: Value = net::get_json("https://meta.fabricmc.net/v2/versions/installer")?;
    fabric_installer_versions_from(&list)
}

pub fn fabric_installer_versions_from(list: &Value) -> Result<Vec<BuildInfo>, Error> {
    let list = lista_de::<FabricInstallerRaw>(list, "fabric")?;
    Ok(list
        .into_iter()
        .map(|e| BuildInfo {
            // aquí el TS no llama a `String()`: un campo ausente desaparece del JSON
            id: e.version,
            channel: Some(canal(e.stable.as_ref()).to_string()),
        })
        .collect())
}

pub fn fabric_download(
    mc: &str,
    loader_build: Option<&str>,
    installer_build: Option<&str>,
) -> Result<EngineDownload, Error> {
    let loader = match loader_build {
        Some(l) => l.to_string(),
        None => latest_version("https://meta.fabricmc.net/v2/versions/loader", "version")?,
    };
    let installer = match installer_build {
        Some(i) => i.to_string(),
        None => latest_version("https://meta.fabricmc.net/v2/versions/installer", "version")?,
    };
    Ok(fabric_download_from(mc, &loader, &installer))
}

/// el loader y el installer van **sin** escapar, tal como los espera la meta API
pub fn fabric_download_from(mc: &str, loader: &str, installer: &str) -> EngineDownload {
    EngineDownload {
        url: format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{loader}/{installer}/server/jar",
            url::encode_uri_component(mc)
        ),
        sha256: None,
        filename: format!("fabric-server-{mc}.jar"),
        installer: None,
        resolved: format!("loader {loader}"),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuiltLoaderRaw {
    pub version: Option<Value>,
    pub loader: Option<FabricLoaderVersion>,
}

pub fn quilt_loader_versions(mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let destino = format!(
        "https://meta.quiltmc.org/v3/versions/loader/{}",
        url::encode_uri_component(mc)
    );
    let list: Value = net::get_json(&destino)?;
    quilt_loader_versions_from(&list)
}

pub fn quilt_loader_versions_from(list: &Value) -> Result<Vec<BuildInfo>, Error> {
    let list = lista_de::<QuiltLoaderRaw>(list, "quilt")?;
    Ok(list
        .into_iter()
        .map(|e| BuildInfo {
            id: Some(
                e.loader
                    .and_then(|l| l.version)
                    .filter(|v| !v.is_null())
                    .unwrap_or_else(|| Value::String(js::string(e.version.as_ref()))),
            ),
            channel: None,
        })
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuiltInstallerRaw {
    pub version: Option<Value>,
}

pub fn quilt_installer_versions() -> Result<Vec<BuildInfo>, Error> {
    let list: Value = net::get_json("https://meta.quiltmc.org/v3/versions/installer")?;
    quilt_installer_versions_from(&list)
}

pub fn quilt_installer_versions_from(list: &Value) -> Result<Vec<BuildInfo>, Error> {
    let list = lista_de::<QuiltInstallerRaw>(list, "quilt")?;
    Ok(list
        .into_iter()
        .map(|e| BuildInfo {
            id: e.version,
            channel: None,
        })
        .collect())
}

pub fn quilt_download(
    mc: &str,
    loader_build: Option<&str>,
    installer_build: Option<&str>,
) -> Result<EngineDownload, Error> {
    let loader = match loader_build {
        Some(l) => l.to_string(),
        None => latest_version("https://meta.quiltmc.org/v3/versions/loader", "version")?,
    };
    let installer = match installer_build {
        Some(i) => i.to_string(),
        None => latest_version("https://meta.quiltmc.org/v3/versions/installer", "version")?,
    };
    Ok(quilt_download_from(mc, &loader, &installer))
}

pub fn quilt_download_from(mc: &str, loader: &str, installer: &str) -> EngineDownload {
    EngineDownload {
        url: format!(
            "https://meta.quiltmc.org/v3/versions/loader/{}/{loader}/{installer}/server/jar",
            url::encode_uri_component(mc)
        ),
        sha256: None,
        filename: format!("quilt-server-{mc}.jar"),
        installer: None,
        resolved: format!("loader {loader}"),
    }
}

fn canal(stable: Option<&Value>) -> &'static str {
    if stable == Some(&Value::Bool(false)) {
        "beta"
    } else {
        "stable"
    }
}

fn lista_de<T: serde::de::DeserializeOwned>(list: &Value, api: &str) -> Result<Vec<T>, Error> {
    serde_json::from_value(list.clone())
        .map_err(|e| Error::Mensaje(format!("respuesta de {api} inesperada: {e}")))
}

// --- Forge ---

/// builds de Forge para una versión de MC (del maven-metadata; nueva primero)
pub fn forge_builds(mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let xml = net::get_text(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
    )?;
    forge_builds_from(&xml, mc)
}

/// el regex del maven-metadata.
///
/// El prefijo va **sin escapar**: con `mc = "1.20.1"` los puntos son comodines,
/// así que también entra `1x20y1-47.2.0`. Y el `Set` quita duplicados conservando
/// la primera aparición para, **después**, invertir la lista: el resultado es el
/// orden inverso de las primeras apariciones, no un orden de versiones.
pub fn forge_builds_from(xml: &str, mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let prefix = format!("{mc}-");
    let re = regex::Regex::new(&format!("<version>{prefix}([^<]+)</version>"))
        .map_err(|e| Error::Mensaje(format!("prefijo inválido para {mc}: {e}")))?;
    let mut vistos: Vec<String> = Vec::new();
    for cap in re.captures_iter(xml) {
        let id = cap[1].to_string();
        if !vistos.contains(&id) {
            vistos.push(id);
        }
    }
    vistos.reverse();
    Ok(vistos
        .into_iter()
        .map(|id| BuildInfo {
            id: Some(Value::String(id)),
            channel: None,
        })
        .collect())
}

/// `recommended` y si no `latest`; `None` significa que no hay Forge para esa MC
pub fn forge_pick_from(promos: &Value, mc: &str) -> Result<Option<String>, Error> {
    // en el TS esto es un TypeError al indexar `undefined`, no un `null`
    let Some(p) = promos.get("promos") else {
        return Err(Error::Mensaje("promos ausente".to_string()));
    };
    if !p.is_object() {
        return Err(Error::Mensaje("promos no es un objeto".to_string()));
    }
    let recomendada = p.get(format!("{mc}-recommended")).and_then(Value::as_str);
    let ultima = p.get(format!("{mc}-latest")).and_then(Value::as_str);
    Ok(recomendada.or(ultima).map(str::to_string))
}

pub fn forge_download(mc: &str, build: Option<&str>) -> Result<EngineDownload, Error> {
    let chosen = match build {
        Some(b) => b.to_string(),
        None => {
            let promos: Value = net::get_json(
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
            )?;
            let elegida = forge_pick_from(&promos, mc)?;
            // el TS comprueba `!chosen`, así que una cadena vacía también es "no hay"
            match elegida {
                Some(c) if !c.is_empty() => c,
                _ => return Err(Error::Mensaje(format!("Forge no soporta {mc}"))),
            }
        }
    };
    Ok(forge_download_from(mc, &chosen))
}

pub fn forge_download_from(mc: &str, chosen: &str) -> EngineDownload {
    let base = format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{chosen}");
    EngineDownload {
        url: format!("{base}/forge-{mc}-{chosen}-installer.jar"),
        sha256: None,
        filename: format!("forge-installer-{mc}.jar"),
        installer: Some(true),
        resolved: chosen.to_string(),
    }
}

// --- NeoForge (1.20.2+; versión = prefijo de MC sin el "1.") ---

const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

#[derive(Debug, Clone, Deserialize)]
pub struct NeoForgeRaw {
    pub versions: Vec<Value>,
}

/// `1.20.2` → `20.2`: el número de NeoForge pierde el primer segmento de la MC
pub fn neoforge_prefix(mc: &str) -> String {
    mc.split('.').skip(1).collect::<Vec<_>>().join(".")
}

pub fn neoforge_builds(mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let data: Value = net::get_json(NEOFORGE_VERSIONS_URL)?;
    neoforge_builds_from(&data, mc)
}

pub fn neoforge_builds_from(data: &Value, mc: &str) -> Result<Vec<BuildInfo>, Error> {
    let mut versiones = neoforge_versions_from(data, mc)?;
    versiones.reverse();
    Ok(versiones
        .into_iter()
        .map(|id| BuildInfo {
            id: Some(Value::String(id)),
            channel: None,
        })
        .collect())
}

/// las versiones que empiezan por el prefijo de esa MC, en el orden de la API
pub fn neoforge_versions_from(data: &Value, mc: &str) -> Result<Vec<String>, Error> {
    let data: NeoForgeRaw = serde_json::from_value(data.clone())
        .map_err(|e| Error::Mensaje(format!("respuesta de neoforge inesperada: {e}")))?;
    let prefijo = format!("{}.", neoforge_prefix(mc));
    Ok(data
        .versions
        .iter()
        .map(|v| js::string(Some(v)))
        .filter(|v| v.starts_with(&prefijo))
        .collect())
}

pub fn neoforge_download(mc: &str, build: Option<&str>) -> Result<EngineDownload, Error> {
    let chosen = match build {
        Some(b) => b.to_string(),
        None => {
            let data: Value = net::get_json(NEOFORGE_VERSIONS_URL)?;
            let candidatas = neoforge_versions_from(&data, mc)?;
            // la **última** de la lista de la API, no la primera
            candidatas
                .last()
                .cloned()
                .ok_or_else(|| Error::Mensaje(format!("NeoForge no soporta {mc}")))?
        }
    };
    Ok(neoforge_download_from(mc, &chosen))
}

pub fn neoforge_download_from(mc: &str, chosen: &str) -> EngineDownload {
    let base = format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{chosen}");
    EngineDownload {
        url: format!("{base}/neoforge-{chosen}-installer.jar"),
        sha256: None,
        filename: format!("neoforge-installer-{mc}.jar"),
        installer: Some(true),
        resolved: chosen.to_string(),
    }
}
