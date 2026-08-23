# Contributing

Issues and pull requests are welcome. PromptLab is an authorized AI security testing tool — keep that scope.

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before changing crates or IPC. Doc index: [docs/README.md](docs/README.md).

## Prerequisites

- Node **18+** (20 or 22 LTS recommended; npm)
- **Rust stable** (1.85+ — `Cargo.toml`'s `rust-version` is stale; do not downgrade)
- [Tauri 2 system deps](https://v2.tauri.app/start/prerequisites/) for your OS

App data lives under `~/.promptlab/` (Windows: `%USERPROFILE%\.promptlab\`), not Tauri `app_data_dir`.

## Run from source

```bash
git clone https://github.com/YangYang-Research/yang-promptlab.git
cd yang-promptlab
npm install
npm run tauri dev
```

The window should show **Connected** (top right) when IPC is up (`health` / `app_info`).

| Command | What it is |
|---------|------------|
| `npm run tauri dev` | Desktop app + Vite. Real backend. |
| `npm run dev` | React UI only. **No IPC** — empty workspace, not mock fixtures. |
| `npm run build` | `tsc --noEmit` then Vite production build |
| `npm test` | Frontend Vitest (`vitest run`) |
| `cargo test -p <crate>` | One Rust crate. Prefer this over `cargo test --workspace`. |

`tauri dev` watches `src-tauri/` and workspace crates. Editing those files retriggers a Rust rebuild and/or webview reload.

## Layout

```
src/              React UI (`src/features/*` → `src/shared/ipc`)
src-tauri/        promptlab-desktop (Tauri commands → AppState → crates)
crates/           engines (core, storage, harness, agent, attack, …)
docs/             living product docs
tests/frontend    Vitest
tests/integration cargo crate `promptlab-integration-tests`
```

UI never talks to the target or the LLM directly. All I/O goes Tauri IPC → Rust → **harness**.

## Where to change things

| Change | Start here |
|--------|------------|
| Wizard, pages, IPC client | `src/features/*`, `src/shared/ipc` |
| Tauri commands | `src-tauri/src/commands/` |
| Target profile / verify | `crates/promptlab-target-profile`, [docs/DISCOVERY.md](docs/DISCOVERY.md) |
| Harness provider | `crates/promptlab-harness/src/providers/`, register in the harness crate. [docs/RUNTIME.md](docs/RUNTIME.md) |
| Yazg / sub-agents / tools | `crates/promptlab-agent`, [docs/YAZG.md](docs/YAZG.md) |
| Scan execute / judge | `crates/promptlab-attack`, `crates/promptlab-judge`, [docs/ATTACK.md](docs/ATTACK.md) |
| Local GGUF / remote route | `crates/promptlab-runtime`, `crates/promptlab-inference` |
| SQLite schema | `crates/promptlab-storage` (migrations) |
| Errors, proxy, `~/.promptlab` paths | `crates/promptlab-core` |

New IPC: add the Rust command, then the TypeScript wrapper in `src/shared/ipc`. Do not call `invoke` from a page ad hoc.

## Pull requests

- Small, reviewable diffs. Match existing style.
- Update living docs when behavior changes (`docs/*.md`, `SCREAMING_SNAKE.md` except `README.md`).
- Do not commit secrets, `.env`, or anything under `~/.promptlab/`.
- Do not revive leftover product surfaces: `/plugins`, `promptlab-plugin-host`, crawl-era `promptlab-discovery` / fingerprint / endpoints. Playwright login is leftover (wizard radios disabled) — see [docs/AUTH.md](docs/AUTH.md).
- Attack catalog and harness probes are for **authorized assessment** only. Do not add payloads aimed at unauthenticated third-party abuse.

## Tests

Frontend: `npm test`.

Rust: `cargo test -p promptlab-core` (and the crate you touched). `cargo test --workspace` is **not** green today — several crates have pre-existing failures or hangs. Do not “fix” unrelated workspace tests in the same PR unless that is the PR.

## License

Contributions are under the same [MIT](LICENSE) license as the rest of the repo.
