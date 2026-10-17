#!/usr/bin/env bash
# OpenBlock — paridad (paper 1.20.4)
set -euo pipefail
cd "$(dirname "$0")"

JAVA="java"
[ -f runtime/java.path ] && JAVA="$(cat runtime/java.path)"
JVM_ARGS="$(cat jvm-args.txt 2>/dev/null || true)"

# forge/neoforge: el installer genera su propio launcher (usa user_jvm_args.txt)
if [ -f run.sh ]; then
  exec bash run.sh nogui
fi

JAR="paper-1.20.4-100.jar"
if [ ! -f "$JAR" ]; then
  echo "Falta $JAR. Ejecuta primero ./install.sh" >&2
  exit 1
fi
exec "$JAVA" $JVM_ARGS -jar "$JAR" --nogui
