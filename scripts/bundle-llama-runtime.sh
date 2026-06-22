#!/usr/bin/env bash
# Download llama.cpp server binary into runtime/ for Tauri resource bundling.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/runtime"
RELEASE="${AISEC_LLAMA_RELEASE:-b9551}"
BINARY_NAME="llama-server"
STAGING="$DEST/.bundle-staging"

mkdir -p "$DEST"
rm -rf "$STAGING"
mkdir -p "$STAGING"

case "$(uname -s)" in
  Darwin)
    arch="$(uname -m)"
    if [ "$arch" = "arm64" ]; then
      archive="llama-${RELEASE}-bin-macos-arm64.tar.gz"
    else
      archive="llama-${RELEASE}-bin-macos-x64.tar.gz"
    fi
    url="https://github.com/ggml-org/llama.cpp/releases/download/${RELEASE}/${archive}"
    curl -fsSL "$url" | tar -xz -C "$STAGING"
    ;;
  Linux)
    arch="$(uname -m)"
    if [ "$arch" = "aarch64" ]; then
      archive="llama-${RELEASE}-bin-ubuntu-arm64.tar.gz"
    else
      archive="llama-${RELEASE}-bin-ubuntu-x64.tar.gz"
    fi
    url="https://github.com/ggml-org/llama.cpp/releases/download/${RELEASE}/${archive}"
    curl -fsSL "$url" | tar -xz -C "$STAGING"
    BINARY_NAME="llama-server"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    archive="llama-${RELEASE}-bin-win-cpu-x64.zip"
    url="https://github.com/ggml-org/llama.cpp/releases/download/${RELEASE}/${archive}"
    zip_path="$STAGING/${archive}"
    curl -fsSL "$url" -o "$zip_path"
    unzip -q "$zip_path" -d "$STAGING"
    BINARY_NAME="llama-server.exe"
    ;;
  *)
    echo "Unsupported build host for llama runtime bundle: $(uname -s)" >&2
    exit 1
    ;;
esac

found="$(find "$STAGING" -type f -name "$BINARY_NAME" | head -n 1 || true)"
if [ -z "$found" ]; then
  echo "llama-server not found in release archive" >&2
  exit 1
fi

cp "$found" "$DEST/$BINARY_NAME"
chmod +x "$DEST/$BINARY_NAME" 2>/dev/null || true
rm -rf "$STAGING"

echo "==> Bundled $DEST/$BINARY_NAME (release ${RELEASE})"
