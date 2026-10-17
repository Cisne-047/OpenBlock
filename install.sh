#!/bin/sh
# Instala el binario de OpenBlock (Rust) en Linux, desde un Release de GitHub.
#
#   curl -fsSL https://raw.githubusercontent.com/Cisne-047/OpenBlock/main/install.sh | sh
#
# No necesita Rust ni Node. Descarga el binario de la última versión y lo deja en
# ~/.local/bin (o donde diga OPENBLOCK_BIN). Si te dice que no está en el PATH, es
# que no lo está: añádelo tú, que este script no edita tus ficheros de shell.
#
# Variables:
#   OPENBLOCK_VERSION  versión a instalar (por defecto, la última: `latest`)
#   OPENBLOCK_BIN      directorio destino (por defecto, ~/.local/bin)
set -eu

REPO="Cisne-047/OpenBlock"
VERSION="${OPENBLOCK_VERSION:-latest}"
DEST="${OPENBLOCK_BIN:-$HOME/.local/bin}"

case "$(uname -s)" in
  Linux) ;;
  *) echo "install.sh es para Linux. En Windows usa install.ps1." >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) ARCH="x86_64" ;;
  *)
    echo "arquitectura no soportada: $(uname -m). Solo hay binarios para x86_64." >&2
    exit 1
    ;;
esac

ASSET="forge-${ARCH}-unknown-linux-gnu"
if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"
else
  URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ASSET}"
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "hace falta curl" >&2
  exit 1
fi

mkdir -p "$DEST"
TMP="${DEST}/forge.descargando"
echo "↓ ${URL}"
curl -fsSL --retry 2 -o "$TMP" "$URL"
chmod +x "$TMP"

# se mueve al final: si la descarga se corta, no queda un `forge` a medias
mv "$TMP" "${DEST}/forge"

echo "✓ forge instalado en ${DEST}/forge"
"${DEST}/forge" --version 2>/dev/null || true
case ":${PATH}:" in
  *":${DEST}:"*) ;;
  *)
    echo
    echo "  ${DEST} no está en tu PATH. Añádelo, por ejemplo:"
    echo "    echo 'export PATH=\"${DEST}:\$PATH\"' >> ~/.profile"
    ;;
esac
echo
echo "  forge help    para ver los comandos"
echo "  forge doctor  para diagnosticar un server que ya tengas"
