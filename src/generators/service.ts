import type { ServerConfig } from "../types.js";

/** Servicio autoarrancable: systemd en Linux, WinSW en Windows. */

export function systemdUnit(cfg: ServerConfig, user: string): string {
  return `[Unit]
Description=OpenBlock server ${cfg.name} (${cfg.engine} ${cfg.mcVersion})
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/openblock/${cfg.name}
User=${user}
ExecStart=/opt/openblock/${cfg.name}/start.sh
Restart=on-failure
RestartSec=10
SuccessExitStatus=0 143
# apagado limpio: stop envía "stop" por rcon antes de SIGTERM
TimeoutStopSec=90

[Install]
WantedBy=multi-user.target
`;
}

export function installServiceSh(cfg: ServerConfig, user: string): string {
  return `#!/usr/bin/env bash
# Instala el servicio systemd (requiere root: sudo ./install-service.sh)
set -euo pipefail
UNIT=/etc/systemd/system/openblock-${cfg.name}.service
sed "s|/opt/openblock/${cfg.name}|$(cd "$(dirname "$0")" && pwd)|g; s|^User=.*|User=${user}|" \\
  openblock.service > "$UNIT"
systemctl daemon-reload
systemctl enable --now "openblock-${cfg.name}"
echo "Servicio instalado: systemctl status openblock-${cfg.name}"
`;
}

export function winswConfig(cfg: ServerConfig): string {
  return `<service>
  <id>openblock-${cfg.name}</id>
  <name>OpenBlock ${cfg.name}</name>
  <description>Servidor de Minecraft ${cfg.engine} ${cfg.mcVersion} forjado con OpenBlock</description>
  <executable>%BASE%\\start.bat</executable>
  <startmode>Automatic</startmode>
  <onfailure action="restart" delay="10 sec"/>
  <stoptimeout>90 sec</stoptimeout>
  <log mode="roll-by-size">
    <sizeThreshold>10240</sizeThreshold>
    <keepFiles>4</keepFiles>
  </log>
</service>
`;
}

export function installServicePs1(cfg: ServerConfig): string {
  return `# Instala el servicio de Windows con WinSW (requiere consola de Administrador)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$winsw = "openblock-${cfg.name}.exe"
if (-not (Test-Path $winsw)) {
  Write-Host "Bajando WinSW..."
  Invoke-WebRequest -Uri "https://github.com/winsw/winsw/releases/download/v2.12.0/WinSW-x64.exe" -OutFile $winsw
}
& ".\\$winsw" install
& ".\\$winsw" start
Write-Host "Servicio instalado: Get-Service openblock-${cfg.name}"
`;
}

export function removeServicePs1(cfg: ServerConfig): string {
  return `# Quita el servicio (consola de Administrador)
& ".\\openblock-${cfg.name}.exe" stop
& ".\\openblock-${cfg.name}.exe" uninstall
`;
}
