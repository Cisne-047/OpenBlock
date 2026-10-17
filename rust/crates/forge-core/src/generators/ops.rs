//! Firewall + backups: scripts listos para copiar o programar, portados de
//! `src/generators/ops.ts`.

use crate::types::ServerConfig;

/// Mundos que entran en el backup (constante del TS).
const WORLDS: &str = "world world_nether world_the_end";

/// Script de firewall para Linux (`firewallSh`).
pub fn firewall_sh(cfg: &ServerConfig) -> String {
    let port = cfg.port;
    let name = &cfg.name;
    format!(
        r#"#!/usr/bin/env bash
# Abre el puerto {port}/tcp del server. Requiere sudo. Detecta el firewall activo.
set -euo pipefail
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then
  ufw allow {port}/tcp comment "OpenBlock {name}"
elif command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then
  firewall-cmd --permanent --add-port={port}/tcp
  firewall-cmd --reload
elif command -v nft >/dev/null; then
  nft add rule inet filter input tcp dport {port} accept 2>/dev/null \
    || echo "Añade manualmente: nft add rule inet filter input tcp dport {port} accept"
else
  echo "Sin firewall detectado (ufw/firewalld/nft). Nada que hacer."
fi
"#
    )
}

/// Script de firewall para Windows (`firewallPs1`).
pub fn firewall_ps1(cfg: &ServerConfig) -> String {
    let port = cfg.port;
    let name = &cfg.name;
    format!(
        r#"# Abre el puerto {port}/tcp en el Firewall de Windows (consola de Administrador)
New-NetFirewallRule -DisplayName "OpenBlock {name}" -Direction Inbound -Protocol TCP -LocalPort {port} -Action Allow
New-NetFirewallRule -DisplayName "OpenBlock {name} UDP" -Direction Inbound -Protocol UDP -LocalPort {port} -Action Allow
"#
    )
}

/// Script de backup para Linux (`backupSh`).
pub fn backup_sh(cfg: &ServerConfig) -> String {
    let keep = cfg.backup.keep;
    format!(
        r#"#!/usr/bin/env bash
# Backup de mundos + configs a backups/, rota los últimos {keep}.
# Recomendado: programar con el timer systemd (openblock-backup.timer) o cron,
# con el server parado o aceptando un autosave pausado.
set -euo pipefail
cd "$(dirname "$0")"
STAMP="$(date +%Y%m%d-%H%M%S)"
mkdir -p backups

tar -czf "backups/backup-$STAMP.tar.gz" \
  {WORLDS} server.properties paper-global.yml paper-world-defaults.yml \
  spigot.yml bukkit.yml plugins 2>/dev/null || \
tar -czf "backups/backup-$STAMP.tar.gz" {WORLDS} server.properties

# rotación
ls -1t backups/backup-*.tar.gz 2>/dev/null | tail -n +$(({keep} + 1)) | xargs -r rm -f
echo "Backup creado: backups/backup-$STAMP.tar.gz"
"#
    )
}

/// Script de backup para Windows (`backupPs1`).
pub fn backup_ps1(cfg: &ServerConfig) -> String {
    let keep = cfg.backup.keep;
    format!(
        r#"# Backup de mundos + configs a backups\, rota los últimos {keep}.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
New-Item -ItemType Directory -Force -Path backups | Out-Null
$targets = @("world","world_nether","world_the_end","server.properties","paper-global.yml","paper-world-defaults.yml","spigot.yml","bukkit.yml") | Where-Object {{ Test-Path $_ }}
$dest = "backups\backup-$stamp.zip"
Compress-Archive -Path $targets -DestinationPath $dest
Get-ChildItem backups\backup-*.zip | Sort-Object LastWriteTime -Descending | Select-Object -Skip {keep} | Remove-Item -Force
Write-Host "Backup creado: $dest"
"#
    )
}

/// Unidad systemd del timer de backups (`backupTimer`).
pub fn backup_timer(cfg: &ServerConfig) -> String {
    // `Math.max(1, intervalHours)`: un 0 daría `0/0` y systemd no lo entendería.
    let each = std::cmp::max(1, cfg.backup.interval_hours);
    let name = &cfg.name;
    format!(
        r#"[Unit]
Description=OpenBlock backup {name}

[Timer]
OnCalendar=*-*-* 0/{each}:00:00
Persistent=true

[Install]
WantedBy=timers.target
"#
    )
}
