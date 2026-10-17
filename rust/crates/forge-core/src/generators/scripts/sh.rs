//! Scripts de Linux, portados de `src/generators/scripts.ts`.
//!
//! Plantillas largas: aquí la paridad byte a byte es el único juez posible. Los
//! escapes de bash dentro de un literal de Rust son terreno minado (el TS
//! escribe `\$` y en Rust eso sería una barra literal), así que van en cadenas
//! crudas y con los `$` tal cual.

use crate::catalog::java_major_for;
use crate::plan::DownloadPlan;
use crate::types::{EngineId, ServerConfig};

/// Comilla un valor para bash: `'…'` con las comillas simples escapadas.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Los tres arrays de descargas del `install.sh` (`shDownloads`).
fn sh_downloads(plan: &DownloadPlan) -> String {
    let mut urls = Vec::new();
    let mut dests = Vec::new();
    let mut sums = Vec::new();
    for e in &plan.entries {
        urls.push(shell_quote(&e.url));
        dests.push(shell_quote(&e.dest));
        sums.push(shell_quote(e.script_sum()));
    }
    format!(
        "URLS=({})\nDESTS=({})\nSUMS=({})",
        urls.join(" "),
        dests.join(" "),
        sums.join(" ")
    )
}

/// `start.sh` (`startSh`).
pub fn start_sh(cfg: &ServerConfig, plan: &DownloadPlan) -> String {
    let (name, engine, mc) = (&cfg.name, cfg.engine, &cfg.mc_version);
    let jar = &plan.engine_filename;
    let nogui = if cfg.engine == EngineId::Vanilla {
        "nogui"
    } else {
        "--nogui"
    };
    format!(
        r#"#!/usr/bin/env bash
# OpenBlock — {name} ({engine} {mc})
set -euo pipefail
cd "$(dirname "$0")"

JAVA="java"
[ -f runtime/java.path ] && JAVA="$(cat runtime/java.path)"
JVM_ARGS="$(cat jvm-args.txt 2>/dev/null || true)"

# forge/neoforge: el installer genera su propio launcher (usa user_jvm_args.txt)
if [ -f run.sh ]; then
  exec bash run.sh nogui
fi

JAR="{jar}"
if [ ! -f "$JAR" ]; then
  echo "Falta $JAR. Ejecuta primero ./install.sh" >&2
  exit 1
fi
exec "$JAVA" $JVM_ARGS -jar "$JAR" {nogui}
"#
    )
}

/// `install.sh` (`installSh`).
pub fn install_sh(cfg: &ServerConfig, plan: &DownloadPlan) -> String {
    let name = &cfg.name;
    let major = java_major_for(&cfg.mc_version);
    let downloads = sh_downloads(plan);
    let installer = match &plan.installer {
        Some(i) => {
            let eng = &i.engine;
            let jar = &i.jar;
            format!(
                r#"echo "== Ejecutando installer de {eng} (descarga librerías, tarda) =="
"$JAVA" -jar "{jar}" --installServer
"#
            )
        }
        None => String::new(),
    };
    format!(
        r##"#!/usr/bin/env bash
# OpenBlock install — {name}
# Descarga el engine, plugins/mods y Java si falta. Requiere curl y tar.
set -euo pipefail
cd "$(dirname "$0")"

JAVA_MAJOR={major}
JAVA="java"

need_java() {{
  command -v java >/dev/null 2>&1 || return 0
  V="$(java -version 2>&1 | sed -n 's/.*version "\(1\.\)\?\([0-9]*\).*/\2/p')"
  [ "$V" != "$JAVA_MAJOR" ] && return 0
  return 1
}}

if need_java; then
  echo "== Bajando Temurin JDK $JAVA_MAJOR (runtime encapsulado) =="
  ARCH="$(uname -m)"; [ "$ARCH" = "aarch64" ] && A=aarch64 || A=x64
  TARBALL="OpenJDK{major}-jdk_$A.tar.gz"
  curl -fL --retry 2 -o "$TARBALL" \
    "https://api.adoptium.net/v3/binary/latest/$JAVA_MAJOR/ga/linux/$A/jdk/hotspot/normal/eclipse"
  mkdir -p runtime
  tar -xzf "$TARBALL" -C runtime
  ROOT="$(find runtime -maxdepth 1 -type d -name 'jdk*' | head -1)"
  chmod +x "$ROOT/bin/java" "$ROOT/bin/keytool" 2>/dev/null || true
  echo "$ROOT/bin/java" > runtime/java.path
  JAVA="$(cat runtime/java.path)"
  rm -f "$TARBALL"
fi

mkdir -p plugins mods
{downloads}
for i in "${{!URLS[@]}}"; do
  URL="${{URLS[$i]}}"; DEST="${{DESTS[$i]}}"; SUM="${{SUMS[$i]}}"
  [ -z "$URL" ] && continue
  echo "== Bajando ${{DEST##*/}} =="
  mkdir -p "$(dirname "$DEST")"
  curl -fL --retry 2 -o "$DEST" "$URL"
  if [ -n "$SUM" ]; then
    # los hashes de Modrinth son sha512 (64+ chars); los de PaperMC, sha256
    if [ "${{#SUM}}" -gt 64 ]; then CHK=sha512sum; else CHK=sha256sum; fi
    echo "$SUM  $DEST" | "$CHK" -c - >/dev/null \
      || {{ echo "Hash no coincide: $DEST" >&2; exit 1; }}
  fi
done

{installer}
chmod +x start.sh
echo "== Listo. Arranca con: ./start.sh =="
"##
    )
}

/// `jvm-args.txt` (`jvmArgsTxt`).
pub fn jvm_args_txt(line: &str) -> String {
    format!("{line}\n")
}

/// `user_jvm_args.txt` para Forge (`userJvmArgsTxt`).
pub fn user_jvm_args_txt(line: &str) -> String {
    format!("{line}\n")
}
