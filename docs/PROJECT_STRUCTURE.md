# PromptLab Project Structure

Bootstrap layout for the PromptLab desktop application. Business logic crates and features are intentionally omitted.

## Repository Tree

```
promptlab/
├── Cargo.toml                  # Rust workspace root
├── package.json                # Frontend + Tauri CLI scripts
├── vite.config.ts              # Vite dev/build configuration
├── vitest.config.ts            # Frontend unit test runner
├── tsconfig.json               # TypeScript (React app)
├── tsconfig.node.json          # TypeScript (tooling configs)
├── index.html                  # Vite entry HTML
│
├── src/                        # React + TypeScript frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── vite-env.d.ts
│   ├── app/
│   │   ├── AppShell.tsx
│   │   └── providers/
│   │       └── AppProviders.tsx
│   ├── shared/
│   │   ├── errors/             # AppError + ErrorBoundary
│   │   ├── ipc/                # Typed Tauri invoke wrappers
│   │   └── logging/            # Frontend logger
│   └── styles/
│       └── global.css
│
├── src-tauri/                  # Tauri + Rust backend shell
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/           # IPC commands (bootstrap only)
│       ├── error.rs              # CommandError envelope
│       ├── logging.rs            # tracing bootstrap
│       └── state.rs              # AppState
│
├── crates/
│   ├── promptlab-core/             # Shared Rust foundations
│   │   └── src/
│   │       ├── error.rs
│   │       └── logging.rs
│   ├── promptlab-fingerprint/      # AI endpoint provider fingerprinting
│   ├── promptlab-plugin-host/      # Plugin manager, sandbox, permissions
│   └── promptlab-storage/          # SQLite + sqlx + repositories
│
├── packages/
│   ├── plugin-sdk-python/      # Python plugin SDK
│   └── plugin-sdk-js/          # JavaScript (Node) plugin SDK
│
├── docs/
│   ├── ARCHITECTURE.md
│   ├── DATABASE.md
│   ├── PLUGINS.md
│   └── PROJECT_STRUCTURE.md
│
├── plugins/
│   ├── README.md
│   ├── _template/
│   │   ├── promptlab-plugin.toml
│   │   └── plugin.py
│   └── samples/                # Reference plugins (4 types × Python/JS)
│
└── tests/
    ├── integration/            # Rust integration tests (workspace member)
    │   ├── Cargo.toml
    │   └── tests/
    │       └── core_smoke.rs
    └── frontend/               # Vitest unit tests
        ├── errors.test.ts
        └── logger.test.ts
```

## Cargo Workspace

| Crate | Path | Purpose |
|-------|------|---------|
| `promptlab-core` | `crates/promptlab-core` | Shared error + logging frameworks |
| `promptlab-storage` | `crates/promptlab-storage` | SQLite persistence, migrations, repositories |
| `promptlab-discovery` | `crates/promptlab-discovery` | Attack-surface discovery engine (crawl, API, GraphQL, OpenAPI, AI) |
| `promptlab-core` | `crates/promptlab-core` | Shared errors, logging |
| `promptlab-attack` | `crates/promptlab-attack` | AI security attack framework |
| `promptlab-payload` | `crates/promptlab-payload` | Payload library, mutations, generation pipeline |
| `promptlab-models` | `crates/promptlab-models` | Local GGUF model manager, llama.cpp runtime |
| `promptlab-judge` | `crates/promptlab-judge` | AI judge engine — rule, regex, LLM consensus |
| `promptlab-report` | `crates/promptlab-report` | Report generation — HTML, PDF, JSON, SARIF |
| `promptlab-fingerprint` | `crates/promptlab-fingerprint` | AI endpoint provider fingerprinting |
| `promptlab-plugin-host` | `crates/promptlab-plugin-host` | Plugin lifecycle, sandbox, permissions |
| `promptlab-auth` | `crates/promptlab-auth` | Authentication engine (Playwright sessions) |
| `promptlab-desktop` | `src-tauri` | Tauri application shell |
| `promptlab-integration-tests` | `tests/integration` | Cross-crate smoke tests |

## Bootstrap IPC Commands

| Command | Description |
|---------|-------------|
| `health` | Returns `{ status, version }` |
| `app_info` | Returns static app metadata |

## Development

```bash
# Install frontend dependencies
npm install

# Run web UI only
npm run dev

# Run desktop app (requires Rust toolchain + platform deps)
npm run tauri dev

# Typecheck + build frontend
npm run build

# Run tests
cargo test
npm test
```

## Frameworks

### Logging

- **Rust:** `tracing` + `tracing-subscriber` + `tracing-appender` via `promptlab-core::logging`
- **Frontend:** `createLogger()` in `src/shared/logging` with `VITE_LOG_LEVEL` support

### Error Handling

- **Rust:** `PromptLabError` / `ErrorCode` in `promptlab-core`; `CommandError` IPC envelope in `src-tauri`
- **Frontend:** `AppError` type, `toAppError()`, and `ErrorBoundary` in `src/shared/errors`

## Next Steps (Not Implemented)

Future crates and modules described in `docs/ARCHITECTURE.md`:

- Security engines (`promptlab-engine-*`)
- Orchestrator, storage, vault, plugin host
- Feature modules under `src/features/`
