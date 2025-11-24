#!/bin/bash

set -e

# Detect OS
OS="$(uname -s)"
case "${OS}" in
    Linux*)     OS=linux;;
    Darwin*)    OS=darwin;;
    MINGW*)     OS=windows;;
    *)          echo "Unsupported OS: ${OS}"; exit 1;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64)    ARCH=amd64;;
    aarch64)   ARCH=arm64;;
    arm64)     ARCH=arm64;;
    *)         echo "Unsupported architecture: ${ARCH}"; exit 1;;
esac

# Determine asset name
if [ "$OS" = "windows" ]; then
    ASSET_NAME="craft-${OS}-${ARCH}.exe"
else
    ASSET_NAME="craft-${OS}-${ARCH}"
fi

# GitHub Release URL (latest)
DOWNLOAD_URL="https://github.com/craftpkg/craft/releases/latest/download/${ASSET_NAME}"

echo "Downloading Craft for ${OS}/${ARCH}..."
if ! curl -L -o craft "${DOWNLOAD_URL}" --fail; then
    echo "Error: Failed to download ${ASSET_NAME} from ${DOWNLOAD_URL}"
    echo "Please ensure the release exists and the asset name is correct."
    exit 1
fi

chmod +x craft

echo "Installing to /usr/local/bin (requires sudo)..."
if [ -w /usr/local/bin ]; then
    mv craft /usr/local/bin/craft
else
    sudo mv craft /usr/local/bin/craft
fi

echo "✅ Craft installed successfully!"
echo "Try running: craft --help"
