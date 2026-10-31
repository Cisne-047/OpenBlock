//! `forge versions` y `forge builds` — listados en vivo antes de instalar nada.
//!
//! Elegir a ciegas es la parte más incómoda de montar un server: uno no sabe qué
//! versiones de MC existen hoy ni qué builds hay del engine. Estos dos comandos
//! lo enseñan sin tocar disco: `versions` las releases de Mojang, `builds` las
//! compilaciones del engine que se le pase. El guided ya pregunta la build exacta;
//! esto es lo mismo para quien va con flags.

use crate::ExitCode;
use forge_api::engines::{
    fabric_loader_versions, forge_builds, neoforge_builds, paper_builds, purpur_builds,
    quilt_loader_versions,
};
use forge_core::EngineId;

/// `forge versions [n]` — las últimas releases de Minecraft, la más nueva primero.
///
/// La primera línea marca cuál es `latest.release`, que es el default de `--mc`:
/// así se ve de un vistazo qué se instalaría si no se dice nada.
pub fn versions_cmd(args: &[String]) -> ExitCode {
    let n = args
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(12);
    let manifest = match forge_api::mojang::version_manifest() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("No se pudo consultar Mojang: {e}");
            return ExitCode::FAILURE;
        }
    };
    let releases = forge_api::mojang::filtrar_releases(manifest, n);
    let latest = &manifest.latest.release;
    println!("Releases de Minecraft (más nuevas primero):");
    for r in &releases {
        if r == latest {
            println!("  {r}  ← latest.release (el default de --mc)");
        } else {
            println!("  {r}");
        }
    }
    println!(
        "Snapshot más reciente: {}  (el plan solo acepta releases)",
        manifest.latest.snapshot
    );
    ExitCode::SUCCESS
}

/// `forge builds --engine E --mc V [n]` — builds del engine, la más nueva primero.
///
/// Paper/Folia/Purpur numeran builds; Forge/NeoForge versionan el loader; Fabric
/// y Quilt versionan el loader (con `--installer-build` aparte). El número que se
/// ve aquí es el que acepta `--build` (o `--loader-build` en fabric/quilt).
pub fn builds_cmd(args: &[String]) -> ExitCode {
    let mut engine: Option<EngineId> = None;
    let mut mc: Option<String> = None;
    let mut n: Option<usize> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--engine" if i + 1 < args.len() => {
                engine = EngineId::parse(&args[i + 1]);
                i += 2;
            }
            "--mc" if i + 1 < args.len() => {
                mc = Some(args[i + 1].clone());
                i += 2;
            }
            otro => {
                n = otro.parse::<usize>().ok();
                i += 1;
            }
        }
    }
    let Some(engine) = engine else {
        eprintln!("uso: forge builds --engine paper|purpur|folia|fabric|quilt|forge|neoforge --mc <versión> [n]");
        return ExitCode::from(2);
    };
    let n = n.unwrap_or(12);
    let engine = if engine == EngineId::Vanilla {
        eprintln!(
            "vanilla no tiene builds: la versión de MC es la versión. `forge versions` las lista."
        );
        return ExitCode::from(2);
    } else {
        engine
    };
    // sin --mc, la latest: es el default con el que se instalaría
    let mc = match mc {
        Some(m) => m,
        None => match forge_api::mojang::latest_release() {
            Ok(l) => l.to_string(),
            Err(e) => {
                eprintln!("No se pudo consultar Mojang para el default de --mc: {e}");
                eprintln!("Pásalo a mano: forge builds --engine {engine} --mc 1.21.4");
                return ExitCode::FAILURE;
            }
        },
    };

    let builds = match listar_builds(engine, &mc) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("No se pudieron listar las builds de {engine} {mc}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if builds.is_empty() {
        eprintln!("{engine} no tiene builds para {mc} (¿existe esa versión para este engine?)");
        return ExitCode::FAILURE;
    }
    let campo = matches!(engine, EngineId::Fabric | EngineId::Quilt)
        .then_some("--loader-build")
        .unwrap_or("--build");
    println!("Builds de {engine} para MC {mc} (más nuevas primero; pásalas con {campo}):");
    for b in builds.iter().take(n) {
        let canal = b
            .channel
            .as_deref()
            .map(|c| format!("  ({c})"))
            .unwrap_or_default();
        println!("  {}{canal}", b.id_str());
    }
    if builds.len() > n {
        println!(
            "  … y {} más (forge builds --engine … {})",
            builds.len() - n,
            builds.len()
        );
    }
    ExitCode::SUCCESS
}

/// El switch de listado, un sitio por engine (`buildsFor` del TS).
fn listar_builds(engine: EngineId, mc: &str) -> Result<Vec<forge_api::engines::BuildInfo>, String> {
    match engine {
        EngineId::Paper => Ok(paper_builds("paper", mc)),
        EngineId::Folia => Ok(paper_builds("folia", mc)),
        EngineId::Purpur => purpur_builds(mc).map_err(|e| e.to_string()),
        EngineId::Fabric => fabric_loader_versions(mc).map_err(|e| e.to_string()),
        EngineId::Quilt => quilt_loader_versions(mc).map_err(|e| e.to_string()),
        EngineId::Forge => forge_builds(mc).map_err(|e| e.to_string()),
        EngineId::Neoforge => neoforge_builds(mc).map_err(|e| e.to_string()),
        EngineId::Vanilla => Ok(Vec::new()),
    }
}
