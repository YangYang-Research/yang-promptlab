#!/usr/bin/env bash
# Generate PromptLab release notes from commits since the previous release tag.
#
# Range priority:
#   1. Previous v* tag (exclude current v${VERSION}) — full changelog vs last release
#   2. merge-base with origin/main
#   3. Root commit
#
# Usage:
#   ./scripts/generate-release-notes.sh [out.md]
# Env (optional):
#   PR_NUMBER, HEAD_SHA, VERSION, BASE (force range start)

set -euo pipefail

OUT="${1:-release-notes.md}"
PR_NUMBER="${PR_NUMBER:-}"
HEAD_SHA="${HEAD_SHA:-$(git rev-parse --short HEAD)}"
VERSION="${VERSION:-}"
BASE="${BASE:-}"

if [[ -z "$VERSION" && -f src-tauri/tauri.conf.json ]]; then
  VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version" 2>/dev/null || true)"
fi
VERSION="${VERSION:-0.0.0}"
CURRENT_TAG="v${VERSION}"

git fetch origin main --depth=200 2>/dev/null || true
git fetch --tags --force 2>/dev/null || true

if [[ -z "$BASE" ]]; then
  # Prefer previous stable tag (vX.Y.Z only — skip v0.1.0-pr.*, etc.) so notes
  # cover everything since last release, not only post-merge-to-main commits.
  BASE="$(
    git tag -l 'v*' --sort=-v:refname \
      | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
      | grep -Ev "^${CURRENT_TAG}$" \
      | head -n1 \
      || true
  )"
fi
if [[ -z "$BASE" ]] && git rev-parse --verify origin/main >/dev/null 2>&1; then
  BASE="$(git merge-base HEAD origin/main 2>/dev/null || true)"
fi
if [[ -z "$BASE" ]]; then
  BASE="$(git rev-list --max-parents=0 HEAD | tail -n1)"
fi

COMMITS_FILE="$(mktemp)"
trap 'rm -f "$COMMITS_FILE"' EXIT
git log --no-merges --pretty=format:'%s' "${BASE}..HEAD" 2>/dev/null >"$COMMITS_FILE" || true

features_file="$(mktemp)"
fixes_file="$(mktemp)"
changes_file="$(mktemp)"
trap 'rm -f "$COMMITS_FILE" "$features_file" "$fixes_file" "$changes_file"' EXIT
: >"$features_file"
: >"$fixes_file"
: >"$changes_file"

strip_prefix() {
  local s="$1"
  # drop conventional-commit type/scope prefix: feat(foo)!: msg
  echo "$s" | sed -E 's/^[a-zA-Z]+(\([^)]*\))?\!?:[[:space:]]*//'
}

# Collapse noisy CI-only release plumbing into a short summary bullet.
is_release_plumbing() {
  local lower="$1"
  case "$lower" in
    ci\(*|chore\(release\)*|chore\(ci\)*|ci:*|chore:*)
      case "$lower" in
        *release*|*workflow*|*r2*|*manifest*|*apple_certificate*|*tauri-action*|*platform*|*rerun*|*retry*)
          return 0
          ;;
      esac
      ;;
  esac
  return 1
}

plumbing_count=0
count=0

while IFS= read -r msg || [[ -n "$msg" ]]; do
  [[ -z "$msg" ]] && continue
  count=$((count + 1))
  lower="$(echo "$msg" | tr '[:upper:]' '[:lower:]')"
  if is_release_plumbing "$lower"; then
    plumbing_count=$((plumbing_count + 1))
    continue
  fi
  # skip bare version-bump chore
  case "$lower" in
    chore:[\ ]release[\ ]v[0-9]*|chore:[\ ]release[\ ][0-9]*) continue ;;
  esac
  bullet="- $(strip_prefix "$msg")"
  case "$lower" in
    fix\(*|fix:*|bugfix\(*|bugfix:*|hotfix\(*|hotfix:*)
      echo "$bullet" >>"$fixes_file"
      ;;
    feat\(*|feat:*|feature\(*|feature:*)
      echo "$bullet" >>"$features_file"
      ;;
    *)
      echo "$bullet" >>"$changes_file"
      ;;
  esac
done <"$COMMITS_FILE"

if [[ "$plumbing_count" -gt 0 ]]; then
  echo "- release CI: platform job stages, manifest/R2 publish gate, macOS signing hardening (${plumbing_count} commits)" >>"$changes_file"
fi

overview="PromptLab **v${VERSION}**"
if [[ -n "$PR_NUMBER" ]]; then
  overview+=" draft from PR #${PR_NUMBER}"
fi
overview+=" (\`${HEAD_SHA}\`)."
overview+=" Changes since **${BASE}** (**${count}** commit(s))."

render_list() {
  local file="$1"
  if [[ ! -s "$file" ]]; then
    echo "_None in this range._"
  else
    cat "$file"
  fi
}

{
  echo "## Overview"
  echo "$overview"
  echo
  echo "## New Features / What's Changed"
  if [[ -s "$features_file" ]]; then
    echo
    echo "### Features"
    render_list "$features_file"
  fi
  if [[ -s "$changes_file" ]]; then
    echo
    echo "### Other changes"
    render_list "$changes_file"
  fi
  if [[ ! -s "$features_file" && ! -s "$changes_file" ]]; then
    echo
    echo "_None in this range._"
  fi
  echo
  echo "## Bug Fixes"
  echo
  render_list "$fixes_file"
} >"$OUT"

echo "Wrote $OUT ($count commits since $BASE)"
