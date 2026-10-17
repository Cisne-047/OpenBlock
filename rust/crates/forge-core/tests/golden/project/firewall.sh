#!/usr/bin/env bash
# Abre el puerto 25565/tcp del server. Requiere sudo. Detecta el firewall activo.
set -euo pipefail
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then
  ufw allow 25565/tcp comment "OpenBlock paridad"
elif command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then
  firewall-cmd --permanent --add-port=25565/tcp
  firewall-cmd --reload
elif command -v nft >/dev/null; then
  nft add rule inet filter input tcp dport 25565 accept 2>/dev/null \
    || echo "Añade manualmente: nft add rule inet filter input tcp dport 25565 accept"
else
  echo "Sin firewall detectado (ufw/firewalld/nft). Nada que hacer."
fi
