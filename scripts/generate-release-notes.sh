#!/usr/bin/env bash
# Generate PromptLab release notes from commits since merge-base with main
# (or since the latest v* tag / root commit as fallback).
#
# Usage:
#   ./scripts/generate-release-notes.sh [out.md]
# Env (optional):
#   PR_NUMBER, HEAD_SHA, VERSION

set -euo pipefail

OUT="${1:-release-notes.md}"
PR_NUMBER="${PR_NUMBER:-}"
HEAD_SHA="${HEAD_SHA:-$(git rev-parse --short HEAD)}"
VERSION="${VERSION:-}"

if [[ -z "$VERSION" && -f src-tauri/tauri.conf.json ]]; then
  VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version" 2>/dev/null || true)"
fi
VERSION="${VERSION:-0.0.0}"

git fetch origin main --depth=200 2>/dev/null || true

BASE=""
if git rev-parse --verify origin/main >/dev/null 2>&1; then
  BASE="$(git merge-base HEAD origin/main 2>/dev/null || true)"
fi
if [[ -z "$BASE" ]]; then
  BASE="$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)"
fi
if [[ -z "$BASE" ]]; then
  BASE="$(git rev-list --max-parents=0 HEAD | tail -n1)"
fi

mapfile -t COMMITS < <(git log --no-merges --pretty=format:'%s' "${BASE}..HEAD" 2>/dev/null || true)

features=()
fixes=()
changes=()

strip_prefix() {
  local s="$1"
  # drop conventional-commit type/scope prefix: feat(foo)!: msg
  echo "$s" | sed -E 's/^[a-zA-Z]+(\([^)]*\))?\!?:[[:space:]]*//'
}

for msg in "${COMMITS[@]+"${COMMITS[@]}"}"; do
  [[ -z "$msg" ]] && continue
  # skip pure chore(ci) noise that only touches release plumbing if desired — keep all for transparency
  lower="$(echo "$msg" | tr '[:upper:]' '[:lower:]')"
  bullet="- $(strip_prefix "$msg")"
  if [[ "$lower" =~ ^(fix|bugfix|hotfix)(\(|:|!) ]]; then
    fixes+=("$bullet")
  elif [[ "$lower" =~ ^(feat|feature)(\(|:|!) ]]; then
    features+=("$bullet")
  else
    changes+=("$bullet")
  fi
done

count="${#COMMITS[@]}"
overview="PromptLab **v${VERSION}**"
if [[ -n "$PR_NUMBER" ]]; then
  overview+=" draft from PR #${PR_NUMBER}"
fi
overview+=" (\`${HEAD_SHA}\`)."
overview+=" Auto-generated from **${count}** commit(s) since \`${BASE:0:12}\`."

render_list() {
  local -n arr=$1
  if ((${#arr[@]} == 0)); then
    echo "_None in this range._"
  else
    printf '%s\n' "${arr[@]}"
  fi
}

{
  echo "## Overview"
  echo "$overview"
  echo
  echo "## New Features / What's Changed"
  if ((${#features[@]})); then
    echo
    echo "### Features"
    render_list features
  fi
  if ((${#changes[@]})); then
    echo
    echo "### Other changes"
    render_list changes
  fi
  if ((${#features[@]} == 0 && ${#changes[@]} == 0)); then
    echo
    echo "_None in this range._"
  fi
  echo
  echo "## Bug Fixes"
  echo
  render_list fixes
} >"$OUT"

echo "Wrote $OUT ($count commits since $BASE)"
