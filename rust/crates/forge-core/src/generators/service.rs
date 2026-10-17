//! Servicio autoarrancable: systemd en Linux, WinSW en Windows. Portado de
//! `src/generators/service.ts`.

use crate::types::ServerConfig;

/// Unidad systemd del server (`systemdUnit`).
pub fn systemd_unit(cfg: &ServerConfig, user: &str) -> String {
    let (name, engine, mc) = (&cfg.name, cfg.engine, &cfg.mc_version);
    format!(
        r#"[Unit]
Description=OpenBlock server {name} ({engine} {mc})
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/openblock/{name}
User={user}
ExecStart=/opt/openblock/{name}/start.sh
Restart=on-failure
RestartSec=10
SuccessExitStatus=0 143
# apagado limpio: stop envía "stop" por rcon antes de SIGTERM
TimeoutStopSec=90

[Install]
WantedBy=multi-user.target
"#
    )
}

/// Instalador del servicio systemd (`installServiceSh`).
pub fn install_service_sh(cfg: &ServerConfig, user: &str) -> String {
    let name = &cfg.name;
    format!(
        r#"#!/usr/bin/env bash
# Instala el servicio systemd (requiere root: sudo ./install-service.sh)
set -euo pipefail
UNIT=/etc/systemd/system/openblock-{name}.service
sed "s|/opt/openblock/{name}|$(cd "$(dirname "$0")" && pwd)|g; s|^User=.*|User={user}|" \
  openblock.service > "$UNIT"
systemctl daemon-reload
systemctl enable --now "openblock-{name}"
echo "Servicio instalado: systemctl status openblock-{name}"
"#
    )
}

/// Config de WinSW (`winswConfig`).
pub fn winsw_config(cfg: &ServerConfig) -> String {
    let (name, engine, mc) = (&cfg.name, cfg.engine, &cfg.mc_version);
    format!(
        r#"<service>
  <id>openblock-{name}</id>
  <name>OpenBlock {name}</name>
  <description>Servidor de Minecraft {engine} {mc} forjado con OpenBlock</description>
  <executable>%BASE%\start.bat</executable>
  <startmode>Automatic</startmode>
  <onfailure action="restart" delay="10 sec"/>
  <stoptimeout>90 sec</stoptimeout>
  <log mode="roll-by-size">
    <sizeThreshold>10240</sizeThreshold>
    <keepFiles>4</keepFiles>
  </log>
</service>
"#
    )
}

/// Instalador del servicio de Windows (`installServicePs1`).
pub fn install_service_ps1(cfg: &ServerConfig) -> String {
    let name = &cfg.name;
    format!(
        r#"# Instala el servicio de Windows con WinSW (requiere consola de Administrador)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$winsw = "openblock-{name}.exe"
if (-not (Test-Path $winsw)) {{
  Write-Host "Bajando WinSW..."
  Invoke-WebRequest -Uri "https://github.com/winsw/winsw/releases/download/v2.12.0/WinSW-x64.exe" -OutFile $winsw
}}
& ".\$winsw" install
& ".\$winsw" start
Write-Host "Servicio instalado: Get-Service openblock-{name}"
"#
    )
}

/// Desinstalador del servicio de Windows (`removeServicePs1`).
pub fn remove_service_ps1(cfg: &ServerConfig) -> String {
    let name = &cfg.name;
    format!(
        r#"# Quita el servicio (consola de Administrador)
& ".\openblock-{name}.exe" stop
& ".\openblock-{name}.exe" uninstall
"#
    )
}
