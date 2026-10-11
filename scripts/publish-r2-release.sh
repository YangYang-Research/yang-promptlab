#!/usr/bin/env bash
# Publish PromptLab release artifacts + version.json to yangyang R2 public bucket.
#
# Layout (bucket yangyang-org-public):
#   products/yang-promptlab/manifests/version.json
#   products/yang-promptlab/manifests/v{VERSION}.json
#   products/yang-promptlab/releases/v{VERSION}/{installer}
#
# Required env:
#   R2_ACCOUNT_ID
#   R2_ACCESS_KEY_ID
#   R2_SECRET_ACCESS_KEY
# Optional:
#   R2_BUCKET          (default: yangyang-org-public)
#   CDN_BASE           (default: https://cdn.yyng.icu)
#   VERSION / TAG / ASSET_DIR / MANIFEST
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${VERSION:-$(node -p "require('${ROOT}/src-tauri/tauri.conf.json').version")}"
VERSION="${VERSION#v}"
TAG="${TAG:-v${VERSION}}"
ASSET_DIR="${ASSET_DIR:-${ROOT}/dist-assets}"
MANIFEST="${MANIFEST:-${ROOT}/manifests/version.json}"
R2_BUCKET="${R2_BUCKET:-yangyang-org-public}"
CDN_BASE="${CDN_BASE:-https://cdn.yyng.icu}"
CDN_BASE="${CDN_BASE%/}"

PREFIX_MANIFESTS="products/yang-promptlab/manifests"
PREFIX_RELEASES="products/yang-promptlab/releases/v${VERSION}"

for var in R2_ACCOUNT_ID R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY; do
  if [[ -z "${!var:-}" ]]; then
    echo "error: ${var} is required" >&2
    exit 1
  fi
done

if [[ ! -f "${MANIFEST}" ]]; then
  echo "error: manifest not found: ${MANIFEST}" >&2
  exit 1
fi

if ! command -v aws >/dev/null 2>&1; then
  echo "error: aws CLI required (R2 S3-compatible API)" >&2
  exit 1
fi

ENDPOINT="https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com"
export AWS_ACCESS_KEY_ID="${R2_ACCESS_KEY_ID}"
export AWS_SECRET_ACCESS_KEY="${R2_SECRET_ACCESS_KEY}"
export AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-auto}"
export AWS_EC2_METADATA_DISABLED=true

s3() {
  aws s3 --endpoint-url="${ENDPOINT}" "$@"
}

echo "Publishing PromptLab ${TAG} → s3://${R2_BUCKET}/${PREFIX_RELEASES}/"

shopt -s nullglob
assets=("${ASSET_DIR}"/PromptLab-*)
if [[ ${#assets[@]} -eq 0 ]]; then
  echo "warning: no PromptLab-* assets in ${ASSET_DIR}" >&2
else
  for file in "${assets[@]}"; do
    name="$(basename "${file}")"
    echo "  put ${PREFIX_RELEASES}/${name}"
    s3 cp "${file}" "s3://${R2_BUCKET}/${PREFIX_RELEASES}/${name}" \
      --content-type "application/octet-stream"
  done
fi

echo "  put ${PREFIX_MANIFESTS}/version.json"
s3 cp "${MANIFEST}" "s3://${R2_BUCKET}/${PREFIX_MANIFESTS}/version.json" \
  --content-type "application/json" \
  --cache-control "public, max-age=60"

echo "  put ${PREFIX_MANIFESTS}/v${VERSION}.json"
s3 cp "${MANIFEST}" "s3://${R2_BUCKET}/${PREFIX_MANIFESTS}/v${VERSION}.json" \
  --content-type "application/json" \
  --cache-control "public, max-age=31536000, immutable"

echo "CDN manifest: ${CDN_BASE}/${PREFIX_MANIFESTS}/version.json"
echo "CDN releases: ${CDN_BASE}/${PREFIX_RELEASES}/"
echo "Done."
