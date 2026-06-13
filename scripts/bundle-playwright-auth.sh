#!/usr/bin/env bash
# Bundle Node.js + Playwright Chromium + auth runner for Tauri release resources.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/resources/playwright"
AUTH_DEST="$DEST/auth"
NODE_DEST="$DEST/node"
CRATE_PLAYWRIGHT="$ROOT/crates/aisec-auth/playwright"
NODE_VERSION="${NODE_VERSION:-22.21.0}"

echo "==> Bundling Playwright auth runtime into $DEST"

rm -rf "$DEST"
mkdir -p "$AUTH_DEST" "$NODE_DEST"

cp "$CRATE_PLAYWRIGHT/runner.mjs" "$CRATE_PLAYWRIGHT/package.json" "$CRATE_PLAYWRIGHT/package-lock.json" "$AUTH_DEST/"

echo "==> Installing Playwright npm dependencies"
(
  cd "$AUTH_DEST"
  npm ci --omit=dev
  export PLAYWRIGHT_BROWSERS_PATH="$AUTH_DEST/browsers"
  npx playwright install chromium
)

install_node() {
  local os="$1"
  local arch="$2"
  local url=""
  local extract_dir="$NODE_DEST"

  case "$os" in
    darwin)
      url="https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-darwin-${arch}.tar.gz"
      echo "==> Downloading Node.js ${NODE_VERSION} (${os}-${arch})"
      curl -fsSL "$url" | tar -xz -C "$extract_dir" --strip-components=1
      ;;
    linux)
      url="https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${arch}.tar.gz"
      echo "==> Downloading Node.js ${NODE_VERSION} (${os}-${arch})"
      curl -fsSL "$url" | tar -xz -C "$extract_dir" --strip-components=1
      ;;
    windows)
      url="https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-win-${arch}.zip"
      echo "==> Downloading Node.js ${NODE_VERSION} (${os}-${arch})"
      local zip="$NODE_DEST/node.zip"
      curl -fsSL "$url" -o "$zip"
      unzip -q "$zip" -d "$NODE_DEST"
      rm "$zip"
      mv "$NODE_DEST/node-v${NODE_VERSION}-win-${arch}"/* "$NODE_DEST/"
      rmdir "$NODE_DEST/node-v${NODE_VERSION}-win-${arch}" 2>/dev/null || true
      ;;
    *)
      echo "Unsupported OS for Node bundle: $os" >&2
      exit 1
      ;;
  esac
}

case "$(uname -s)" in
  Darwin)
    arch="$(uname -m)"
    if [ "$arch" = "arm64" ]; then node_arch="arm64"; else node_arch="x64"; fi
    install_node darwin "$node_arch"
    ;;
  Linux)
    arch="$(uname -m)"
    if [ "$arch" = "aarch64" ]; then node_arch="arm64"; else node_arch="x64"; fi
    install_node linux "$node_arch"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    install_node windows x64
    ;;
  *)
    echo "Unsupported build host: $(uname -s)" >&2
    exit 1
    ;;
esac

cat > "$DEST/BUNDLE.txt" <<EOF
AISec bundled Playwright auth runtime
node_version=${NODE_VERSION}
playwright_version=$(node -p "require('$AUTH_DEST/node_modules/playwright/package.json').version")
built_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
host=$(uname -s)-$(uname -m)
EOF

echo "==> Playwright auth bundle ready"
echo "    Node:    $NODE_DEST"
echo "    Runner:  $AUTH_DEST/runner.mjs"
echo "    Browsers: $AUTH_DEST/browsers"
