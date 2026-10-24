#!/bin/bash
#
# Installs the craft binary from GitHub releases.
#
#   CRAFT_VERSION=0.1.0   install that release instead of the latest
#   CRAFT_INSTALL_DIR=... install there instead of /usr/local/bin or ~/.local/bin
#   CRAFT_DOWNLOAD_URL=... download release assets from this base URL (a mirror)

set -euo pipefail

REPOSITORY="craftpkg/craft"

case "$(uname -s)" in
    Linux*)  OS=linux ;;
    Darwin*) OS=darwin ;;
    MINGW*|MSYS*|CYGWIN*) OS=windows ;;
    *) echo "Unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64)  ARCH=amd64 ;;
    aarch64|arm64) ARCH=arm64 ;;
    *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

# Alpine and other musl systems get the statically linked build.
LIBC=""
if [ "$OS" = "linux" ] && ls /lib/ld-musl-* >/dev/null 2>&1; then
    LIBC="-musl"
fi

if [ "$OS" = "windows" ]; then
    ASSET_NAME="craft-${OS}-${ARCH}.exe"
    BINARY_NAME="craft.exe"
else
    ASSET_NAME="craft-${OS}-${ARCH}${LIBC}"
    BINARY_NAME="craft"
fi

if [ -n "${CRAFT_DOWNLOAD_URL:-}" ]; then
    BASE_URL="${CRAFT_DOWNLOAD_URL%/}"
elif [ -n "${CRAFT_VERSION:-}" ]; then
    BASE_URL="https://github.com/${REPOSITORY}/releases/download/${CRAFT_VERSION}"
else
    BASE_URL="https://github.com/${REPOSITORY}/releases/latest/download"
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

echo "Downloading ${ASSET_NAME}..."
if ! curl -fsSL -o "${WORK_DIR}/${ASSET_NAME}" "${BASE_URL}/${ASSET_NAME}"; then
    echo "Error: could not download ${BASE_URL}/${ASSET_NAME}" >&2
    exit 1
fi

if curl -fsSL -o "${WORK_DIR}/${ASSET_NAME}.sha256" "${BASE_URL}/${ASSET_NAME}.sha256"; then
    if command -v sha256sum >/dev/null; then
        CHECK=(sha256sum -c)
    else
        CHECK=(shasum -a 256 -c)
    fi
    if ! (cd "$WORK_DIR" && "${CHECK[@]}" "${ASSET_NAME}.sha256" >/dev/null); then
        echo "Error: ${ASSET_NAME} does not match its published checksum" >&2
        exit 1
    fi
    echo "Checksum verified."
else
    echo "Warning: this release publishes no checksum; skipping verification." >&2
fi

if [ -n "${CRAFT_INSTALL_DIR:-}" ]; then
    INSTALL_DIR="$CRAFT_INSTALL_DIR"
elif [ -w /usr/local/bin ]; then
    INSTALL_DIR=/usr/local/bin
else
    INSTALL_DIR="${HOME}/.local/bin"
fi
mkdir -p "$INSTALL_DIR"

chmod +x "${WORK_DIR}/${ASSET_NAME}"
mv "${WORK_DIR}/${ASSET_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
echo "Installed craft to ${INSTALL_DIR}/${BINARY_NAME}"

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "Add ${INSTALL_DIR} to your PATH to run craft, e.g. export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

"${INSTALL_DIR}/${BINARY_NAME}" --version
