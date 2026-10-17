#!/usr/bin/env bash
# Instala el servicio systemd (requiere root: sudo ./install-service.sh)
set -euo pipefail
UNIT=/etc/systemd/system/openblock-paridad.service
sed "s|/opt/openblock/paridad|$(cd "$(dirname "$0")" && pwd)|g; s|^User=.*|User=minecraft|" \
  openblock.service > "$UNIT"
systemctl daemon-reload
systemctl enable --now "openblock-paridad"
echo "Servicio instalado: systemctl status openblock-paridad"
