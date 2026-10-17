//! Las reglas estáticas del doctor, contra el golden que genera el TypeScript.
//!
//! Las reglas son puras pero miran un directorio y preguntan tres cosas a la
//! máquina. El golden monta un directorio **de verdad** para el TS y aquí se
//! reconstruye el mismo con una vista en memoria: si el port leyera otro fichero,
//! o no leyera ninguno, el hallazgo no saldría y la comparación fallaría.
//!
//! El doble de `javaMajorOf` depende de la ruta (devuelve 21 si ve `jdk21`), que es
//! lo que hace que el caso de `runtime/java.path` compruebe que se lee y se recorta.

use forge_core::doctor::{
    evaluar_estaticas, ContextoEstatico, Finding, Hechos, Severity, VistaServidor, STATIC_RULES,
};
use forge_core::modrinth::ModrinthVersion;
use forge_core::types::EngineId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct Caso {
    nombre: String,
    args: Vec<Value>,
    salida_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Entrada {
    archivos: BTreeMap<String, String>,
    cfg: Cfg,
    hechos: HechosGolden,
}

#[derive(Debug, Deserialize)]
struct Cfg {
    #[serde(rename = "mcVersion")]
    mc_version: String,
    port: u16,
    engine: EngineId,
}

#[derive(Debug, Deserialize)]
struct HechosGolden {
    #[serde(rename = "javaMayor", default)]
    java_mayor: Option<u32>,
    #[serde(rename = "puertoLibre", default)]
    puerto_libre: bool,
    #[serde(rename = "ultimaVersion", default)]
    ultima_version: BTreeMap<String, ModrinthVersion>,
}

#[derive(Debug, Serialize)]
struct ProyFinding {
    severity: &'static str,
    code: String,
    source: String,
    message: String,
    fix: Option<ProyFix>,
}

#[derive(Debug, Serialize)]
struct ProyFix {
    code: String,
    description: String,
    considerable: bool,
}

/// Un directorio de mentira a partir del mapa del golden.
struct VistaMapa(BTreeMap<String, String>);

impl VistaServidor for VistaMapa {
    fn existe(&self, rel: &str) -> bool {
        self.0.contains_key(rel) || self.0.keys().any(|k| k.starts_with(&format!("{rel}/")))
    }

    fn leer(&self, rel: &str) -> Option<String> {
        self.0.get(rel).cloned()
    }

    fn listar(&self, rel: &str) -> Option<Vec<String>> {
        if !self.existe(rel) {
            return None;
        }
        let prefijo = format!("{rel}/");
        let mut hijos: Vec<String> = self
            .0
            .keys()
            .filter_map(|k| k.strip_prefix(&prefijo))
            .map(|resto| resto.split('/').next().unwrap_or_default().to_string())
            .collect();
        // `readdir` de Node no garantiza orden; el doctor solo mira el primero que
        // casa, así que aquí se ordena para que la prueba sea determinista
        hijos.sort();
        hijos.dedup();
        Some(hijos)
    }
}

/// Los hechos inyectados, con el mismo doble que el generador del golden.
struct HechosFijos(HechosGolden);

impl Hechos for HechosFijos {
    fn java_mayor(&self, ruta_java: &str) -> Option<u32> {
        if ruta_java.contains("jdk21") {
            Some(21)
        } else {
            self.0.java_mayor
        }
    }

    fn puerto_libre(&self, _puerto: u16) -> bool {
        self.0.puerto_libre
    }

    fn ultima_version(&self, slug: &str, _mc: &str, _loader: &str) -> Option<ModrinthVersion> {
        self.0.ultima_version.get(slug).cloned()
    }
}

fn proyeccion(f: &Finding) -> ProyFinding {
    ProyFinding {
        severity: f.severity.as_str(),
        code: f.code.clone(),
        source: f.source.clone(),
        message: f.message.clone(),
        fix: f.fix_code.as_ref().map(|code| ProyFix {
            code: code.clone(),
            description: f.fix_description.clone().unwrap_or_default(),
            considerable: f.fix_considerable,
        }),
    }
}

fn golden() -> Value {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/doctor-estaticas.json");
    let txt = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()));
    serde_json::from_str(&txt).expect("el golden debe ser JSON")
}

#[test]
fn las_reglas_estaticas_son_las_mismas_y_en_el_mismo_orden() {
    let codigos: Vec<&str> = STATIC_RULES.iter().map(|r| r.code).collect();
    assert_eq!(
        codigos,
        vec![
            "EULA_FILE",
            "SYNC_CHUNK_WRITES",
            "RCON_PASSWORD",
            "JAVA_MAJOR",
            "PORT_BUSY",
            "ENGINE_JAR_MISSING",
            "PLUGIN_UPDATE"
        ],
        "el orden importa: es el que ve el usuario en el informe"
    );
}

#[test]
fn las_reglas_estaticas_coinciden_con_las_del_ts() {
    let g = golden();
    let casos: Vec<Caso> = serde_json::from_value(g["casos"].clone()).expect("casos");
    assert!(
        casos.len() >= 20,
        "el golden debería traer los casos del generador"
    );
    for c in casos {
        let entrada: Entrada = serde_json::from_value(c.args[0].clone())
            .unwrap_or_else(|e| panic!("entrada del caso «{}» ilegible: {e}", c.nombre));
        let vista = VistaMapa(entrada.archivos);
        let hechos = HechosFijos(entrada.hechos);
        let ctx = ContextoEstatico {
            mc_version: entrada.cfg.mc_version,
            port: entrada.cfg.port,
            engine: entrada.cfg.engine,
            dir: "/server".to_string(),
        };
        let salida: Vec<ProyFinding> = evaluar_estaticas(&vista, &ctx, &hechos)
            .iter()
            .map(proyeccion)
            .collect();
        let json = serde_json::to_string(&salida).expect("serializable");
        assert_eq!(
            json,
            c.salida_json.clone().unwrap_or_else(|| "null".to_string()),
            "caso «{}»",
            c.nombre
        );
    }
}

#[test]
fn el_jar_sale_del_script_de_arranque() {
    let con_bat = VistaMapa(BTreeMap::from([(
        "start.bat".to_string(),
        "set \"JAR=server.jar\"\r\njava -Xmx4G -jar server.jar".to_string(),
    )]));
    assert_eq!(
        forge_core::doctor::jar_name_from_start_script(&con_bat).as_deref(),
        Some("server.jar")
    );

    let con_sh = VistaMapa(BTreeMap::from([(
        "start.sh".to_string(),
        "#!/bin/sh\nJAR=\"paper.jar\"\nexec java -jar \"$JAR\"".to_string(),
    )]));
    assert_eq!(
        forge_core::doctor::jar_name_from_start_script(&con_sh).as_deref(),
        Some("paper.jar")
    );

    // el .sh tiene prioridad porque se mira primero
    let los_dos = VistaMapa(BTreeMap::from([
        ("start.sh".to_string(), "JAR=\"sh.jar\"".to_string()),
        ("start.bat".to_string(), "set \"JAR=bat.jar\"".to_string()),
    ]));
    assert_eq!(
        forge_core::doctor::jar_name_from_start_script(&los_dos).as_deref(),
        Some("sh.jar")
    );

    assert_eq!(
        forge_core::doctor::jar_name_from_start_script(&VistaMapa(BTreeMap::new())),
        None
    );
}

#[test]
fn el_loader_del_doctor_no_es_el_del_plan() {
    // aquí el TS hace `engine === "folia" ? "folia" : "paper"`: un server de fabric
    // pregunta por plugins como si fuera paper
    assert_eq!(
        forge_core::doctor::modrinth_loader_de(EngineId::Folia),
        "folia"
    );
    assert_eq!(
        forge_core::doctor::modrinth_loader_de(EngineId::Paper),
        "paper"
    );
    assert_eq!(
        forge_core::doctor::modrinth_loader_de(EngineId::Fabric),
        "paper"
    );
    assert_eq!(
        forge_core::doctor::modrinth_loader_de(EngineId::Forge),
        "paper"
    );
}

#[test]
fn sin_fix_no_hay_codigo_ni_descripcion() {
    // ENGINE_JAR_MISSING es la única regla estática sin arreglo posible
    let vista = VistaMapa(BTreeMap::new());
    let ctx = ContextoEstatico {
        mc_version: "1.20.4".to_string(),
        port: 25565,
        engine: EngineId::Paper,
        dir: "/server".to_string(),
    };
    let hechos = HechosFijos(HechosGolden {
        java_mayor: Some(17),
        puerto_libre: true,
        ultima_version: BTreeMap::new(),
    });
    let hallazgos = evaluar_estaticas(&vista, &ctx, &hechos);
    let jar = hallazgos
        .iter()
        .find(|f| f.code == "ENGINE_JAR_MISSING")
        .expect("sin nada, falta el jar");
    assert_eq!(jar.severity, Severity::Error);
    assert!(jar.fix_code.is_none());
    assert_eq!(jar.source, "/server");
}
