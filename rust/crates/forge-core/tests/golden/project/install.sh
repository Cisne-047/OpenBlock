#!/usr/bin/env bash
# OpenBlock install — paridad
# Descarga el engine, plugins/mods y Java si falta. Requiere curl y tar.
set -euo pipefail
cd "$(dirname "$0")"

JAVA_MAJOR=17
JAVA="java"

need_java() {
  command -v java >/dev/null 2>&1 || return 0
  V="$(java -version 2>&1 | sed -n 's/.*version "\(1\.\)\?\([0-9]*\).*/\2/p')"
  [ "$V" != "$JAVA_MAJOR" ] && return 0
  return 1
}

if need_java; then
  echo "== Bajando Temurin JDK $JAVA_MAJOR (runtime encapsulado) =="
  ARCH="$(uname -m)"; [ "$ARCH" = "aarch64" ] && A=aarch64 || A=x64
  TARBALL="OpenJDK17-jdk_$A.tar.gz"
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
URLS=('https://example/paper-1.20.4-100.jar' 'https://example/spark.jar')
DESTS=('paper-1.20.4-100.jar' 'plugins/spark.jar')
SUMS=('' '')
for i in "${!URLS[@]}"; do
  URL="${URLS[$i]}"; DEST="${DESTS[$i]}"; SUM="${SUMS[$i]}"
  [ -z "$URL" ] && continue
  echo "== Bajando ${DEST##*/} =="
  mkdir -p "$(dirname "$DEST")"
  curl -fL --retry 2 -o "$DEST" "$URL"
  if [ -n "$SUM" ]; then
    # los hashes de Modrinth son sha512 (64+ chars); los de PaperMC, sha256
    if [ "${#SUM}" -gt 64 ]; then CHK=sha512sum; else CHK=sha256sum; fi
    echo "$SUM  $DEST" | "$CHK" -c - >/dev/null \
      || { echo "Hash no coincide: $DEST" >&2; exit 1; }
  fi
done


chmod +x start.sh
echo "== Listo. Arranca con: ./start.sh =="
