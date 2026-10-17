import type { ServerConfig } from "../types.js";

/** Firewall + backups: scripts listos para copiar/programar. */

export function firewallSh(cfg: ServerConfig): string {
  return `#!/usr/bin/env bash
# Abre el puerto ${cfg.port}/tcp del server. Requiere sudo. Detecta el firewall activo.
set -euo pipefail
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then
  ufw allow ${cfg.port}/tcp comment "OpenBlock ${cfg.name}"
elif command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then
  firewall-cmd --permanent --add-port=${cfg.port}/tcp
  firewall-cmd --reload
elif command -v nft >/dev/null; then
  nft add rule inet filter input tcp dport ${cfg.port} accept 2>/dev/null \\
    || echo "Añade manualmente: nft add rule inet filter input tcp dport ${cfg.port} accept"
else
  echo "Sin firewall detectado (ufw/firewalld/nft). Nada que hacer."
fi
`;
}

export function firewallPs1(cfg: ServerConfig): string {
  return `# Abre el puerto ${cfg.port}/tcp en el Firewall de Windows (consola de Administrador)
New-NetFirewallRule -DisplayName "OpenBlock ${cfg.name}" -Direction Inbound -Protocol TCP -LocalPort ${cfg.port} -Action Allow
New-NetFirewallRule -DisplayName "OpenBlock ${cfg.name} UDP" -Direction Inbound -Protocol UDP -LocalPort ${cfg.port} -Action Allow
`;
}

const WORLDS = `world world_nether world_the_end`;

export function backupSh(cfg: ServerConfig): string {
  return `#!/usr/bin/env bash
# Backup de mundos + configs a backups/, rota los últimos ${cfg.backup.keep}.
# Recomendado: programar con el timer systemd (openblock-backup.timer) o cron,
# con el server parado o aceptando un autosave pausado.
set -euo pipefail
cd "$(dirname "$0")"
STAMP="$(date +%Y%m%d-%H%M%S)"
mkdir -p backups

tar -czf "backups/backup-$STAMP.tar.gz" \\
  ${WORLDS} server.properties paper-global.yml paper-world-defaults.yml \\
  spigot.yml bukkit.yml plugins 2>/dev/null || \\
tar -czf "backups/backup-$STAMP.tar.gz" ${WORLDS} server.properties

# rotación
ls -1t backups/backup-*.tar.gz 2>/dev/null | tail -n +$((${cfg.backup.keep} + 1)) | xargs -r rm -f
echo "Backup creado: backups/backup-$STAMP.tar.gz"
`;
}

export function backupPs1(cfg: ServerConfig): string {
  return `# Backup de mundos + configs a backups\\, rota los últimos ${cfg.backup.keep}.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
New-Item -ItemType Directory -Force -Path backups | Out-Null
$targets = @("world","world_nether","world_the_end","server.properties","paper-global.yml","paper-world-defaults.yml","spigot.yml","bukkit.yml") | Where-Object { Test-Path $_ }
$dest = "backups\\backup-$stamp.zip"
Compress-Archive -Path $targets -DestinationPath $dest
Get-ChildItem backups\\backup-*.zip | Sort-Object LastWriteTime -Descending | Select-Object -Skip ${cfg.backup.keep} | Remove-Item -Force
Write-Host "Backup creado: $dest"
`;
}

export function backupTimer(cfg: ServerConfig): string {
  return `[Unit]
Description=OpenBlock backup ${cfg.name}

[Timer]
OnCalendar=*-*-* 0/${Math.max(1, cfg.backup.intervalHours)}:00:00
Persistent=true

[Install]
WantedBy=timers.target
`;
}
