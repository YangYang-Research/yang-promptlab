# AGENTS.md

## Cursor Cloud specific instructions

PromptLab is a Tauri 2 desktop app: React + TypeScript + Vite frontend (`src/`) in a Rust
backend shell (`src-tauri/`), plus a Rust workspace of library crates (`crates/`).
Standard commands live in `package.json` and `docs/PROJECT_STRUCTURE.md`.

### Toolchain / system requirements (already provisioned in this VM)
- **Rust stable is required (>= 1.85).** `Cargo.toml`'s `rust-version = "1.77"` is inaccurate:
  transitive deps (e.g. `base64ct`) need `edition2024`, so the default toolchain has been set
  to `stable` (`rustup default stable`). Don't downgrade.
- Tauri Linux GUI deps are installed via apt: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`
  (plus `build-essential`, `libssl-dev`, `libxdo-dev`). These are not in the update script.

### Running the app
- Desktop app (real backend): `npm run tauri dev` — launches the native window on `DISPLAY=:1`
  and auto-starts Vite. The top-right indicator shows **"Connected"** when the Rust IPC backend
  (`health` / `app_info`) is reachable.
- Application data root: `~/.promptlab/` (not Tauri app_data_dir).
- Browser-only UI: `npm run dev` (serves `http://localhost:5173`). With no Tauri IPC the UI
  falls back to mock data and shows **"Mock mode"**.

### Headless WebKitGTK rendering caveat (non-obvious)
Under software rendering in this VM, export these before `npm run tauri dev` for stable rendering:
`WEBKIT_DISABLE_DMABUF_RENDERER=1`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`, `LIBGL_ALWAYS_SOFTWARE=1`.
Even so, the WebView may intermittently restart and briefly show the AISec boot/splash screen
(spinning cube). This is an environment rendering artifact, not an app crash — the Rust process
(`target/debug/aisec-desktop`) stays alive and the page re-bootstraps back to "Connected".

### Tauri dev file watching
`tauri dev` watches `src-tauri/` and the workspace crates; editing any workspace file triggers
a Rust recompile and/or a webview reload. Avoid editing repo files while doing GUI demos.

### Tests / build status
- Frontend: `npm test` (Vitest) passes **37/37 tests**, but **one suite fails to load**
  (`tests/frontend/reportDownloads.test.ts`) and `npm run build` (tsc) **fails**, both because of a
  **pre-existing repo bug**, not the environment: the source file
  `src/features/reports/reportDownloads.ts` is imported by `ReportsPage`, `ScanDetailsPage`,
  `ResultsStep`, and the test above, but was **never committed**. Root cause: `.gitignore`'s
  `reports/` rule (last line) recursively ignores `src/features/reports/`, so the file is invisible
  to git and absent on a fresh clone. Consequence in `npm run tauri dev`: the **Scans / Scan-wizard /
  Scan-details / Reports** routes crash the WebView (Vite "Cannot find module" overlay). Note that
  **creating a project navigates to `/scans/new`** (the scan wizard) on success, so it also hits this.
  Safe core flows that work end-to-end: project create/edit/delete (Dashboard/Projects/Project-details)
  and target/discovery actions. To actually fix: un-ignore the file (e.g. `!src/features/reports/`
  or scope the rule to `/reports/`) and commit `reportDownloads.ts`.
- Rust: the workspace **builds** (`cargo build --workspace`). However `cargo test --workspace`
  does NOT fully pass today due to **pre-existing** code/manifest bugs (not environment issues):
  - `aisec-storage` lib test: missing `create` method.
  - `aisec-auth`: uses `tokio::process` without the tokio `process` feature.
  - `aisec-integration-tests`: uses `tracing` without declaring it as a dependency.
  - `aisec-discovery`: `crawler::tests::crawler_respects_max_depth` hangs (network-dependent).
  - One failing test each in `aisec-judge` and `aisec-plugin-host`.
  - Crates whose tests pass cleanly: `aisec-core`, `aisec-payload`, `aisec-models`,
    `aisec-report`, `aisec-fingerprint`. Test individual crates with `cargo test -p <crate>`.
