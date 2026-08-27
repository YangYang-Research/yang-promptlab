#!/usr/bin/env node
/**
 * Generate updates/latest.json from a version + optional local asset directory.
 *
 * Usage:
 *   node scripts/generate-update-manifest.mjs
 *   VERSION=0.2.0 TAG=v0.2.0 ASSET_DIR=./dist-assets node scripts/generate-update-manifest.mjs
 *
 * Env:
 *   VERSION     semver without leading v (default: tauri.conf.json)
 *   TAG         git/release tag (default: v${VERSION})
 *   REPO        owner/name (default: YangYang-Research/yang-promptlab)
 *   ASSET_DIR   directory of built installers; when set, sha256/size are filled
 *   NOTES       release notes (optional)
 *   PUB_DATE    RFC 3339 (default: now UTC)
 *   OUT         output path (default: updates/latest.json)
 */

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function tauriVersion() {
  const conf = JSON.parse(readFileSync(join(ROOT, "src-tauri/tauri.conf.json"), "utf8"));
  return String(conf.version || "0.1.0");
}

function sha256File(path) {
  const hash = createHash("sha256");
  hash.update(readFileSync(path));
  return hash.digest("hex");
}

function findAsset(assetDir, candidates) {
  if (!assetDir || !existsSync(assetDir)) return null;
  const names = readdirSync(assetDir);
  for (const candidate of candidates) {
    const exact = names.find((name) => name === candidate);
    if (exact) return join(assetDir, exact);
  }
  for (const candidate of candidates) {
    const needle = candidate.toLowerCase();
    const fuzzy = names.find((name) => name.toLowerCase() === needle);
    if (fuzzy) return join(assetDir, fuzzy);
  }
  return null;
}

const VERSION = (process.env.VERSION || tauriVersion()).replace(/^v/i, "");
const TAG = process.env.TAG || `v${VERSION}`;
const REPO = process.env.REPO || "YangYang-Research/yang-promptlab";
const ASSET_DIR = process.env.ASSET_DIR ? resolve(process.env.ASSET_DIR) : "";
const NOTES = process.env.NOTES || "Current release.";
const PUB_DATE = process.env.PUB_DATE || new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
const OUT = resolve(process.env.OUT || join(ROOT, "updates/latest.json"));
const BASE = `https://github.com/${REPO}/releases/download/${TAG}`;

const PLATFORMS = [
  {
    key: "darwin-aarch64",
    filenames: [`PromptLab-${VERSION}-darwin-aarch64.dmg`],
  },
  {
    key: "darwin-x86_64",
    filenames: [`PromptLab-${VERSION}-darwin-x86_64.dmg`],
  },
  {
    key: "linux-x86_64",
    filenames: [
      `PromptLab-${VERSION}-linux-x86_64.AppImage`,
      `PromptLab_${VERSION}_amd64.AppImage`,
    ],
  },
  {
    key: "windows-x86_64",
    filenames: [
      `PromptLab-${VERSION}-windows-x64-setup.exe`,
      `PromptLab_${VERSION}_x64-setup.exe`,
      `PromptLab-${VERSION}-windows-x86_64-setup.exe`,
    ],
  },
];

const platforms = {};
for (const spec of PLATFORMS) {
  const filename = spec.filenames[0];
  const local = findAsset(ASSET_DIR, spec.filenames);
  const entry = {
    url: `${BASE}/${local ? local.split(/[/\\]/).pop() : filename}`,
    filename: local ? local.split(/[/\\]/).pop() : filename,
    sha256: "",
    size: 0,
  };
  if (local) {
    entry.sha256 = sha256File(local);
    entry.size = statSync(local).size;
    entry.url = `${BASE}/${entry.filename}`;
  }
  platforms[spec.key] = entry;
}

const manifest = {
  schemaVersion: 1,
  name: "PromptLab",
  version: VERSION,
  notes: NOTES,
  pubDate: PUB_DATE,
  mandatory: false,
  platforms,
};

writeFileSync(OUT, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`wrote ${OUT} version=${VERSION} tag=${TAG}`);
for (const [key, asset] of Object.entries(platforms)) {
  const hash = asset.sha256 ? asset.sha256.slice(0, 12) : "pending";
  console.log(`  ${key}: ${asset.filename} sha256=${hash} size=${asset.size}`);
}
