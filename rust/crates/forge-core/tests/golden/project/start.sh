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
# bucle anti-crash: un `stop` limpio sale (código 0); un crash reintenta a los 5 s.
# OPENBLOCK_AUTORESTART=0 lo desactiva para esa ejecución. El `|| CODE=$?` es para
# que `set -e` no mate el script justo en el crash que queremos capturar.
while true; do
  CODE=0
  "$JAVA" $JVM_ARGS -jar "$JAR" --nogui || CODE=$?
  [ "$CODE" -eq 0 ] && exit 0
  if [ "${OPENBLOCK_AUTORESTART:-1}" != "1" ]; then exit "$CODE"; fi
  echo "== Crash (código $CODE). Reinicio en 5 s; OPENBLOCK_AUTORESTART=0 desactiva ==" >&2
  sleep 5
done
