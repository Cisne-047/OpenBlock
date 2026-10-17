#!/usr/bin/env bash
# Backup de mundos + configs a backups/, rota los últimos 7.
# Recomendado: programar con el timer systemd (openblock-backup.timer) o cron,
# con el server parado o aceptando un autosave pausado.
set -euo pipefail
cd "$(dirname "$0")"
STAMP="$(date +%Y%m%d-%H%M%S)"
mkdir -p backups

tar -czf "backups/backup-$STAMP.tar.gz" \
  world world_nether world_the_end server.properties paper-global.yml paper-world-defaults.yml \
  spigot.yml bukkit.yml plugins 2>/dev/null || \
tar -czf "backups/backup-$STAMP.tar.gz" world world_nether world_the_end server.properties

# rotación
ls -1t backups/backup-*.tar.gz 2>/dev/null | tail -n +$((7 + 1)) | xargs -r rm -f
echo "Backup creado: backups/backup-$STAMP.tar.gz"
