# AISec Project Structure

Bootstrap layout for the AISec desktop application. Business logic crates and features are intentionally omitted.

## Repository Tree

```
aisec/
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
│   ├── aisec-core/             # Shared Rust foundations
│   │   └── src/
│   │       ├── error.rs
│   │       └── logging.rs
│   ├── aisec-fingerprint/      # AI endpoint provider fingerprinting
│   ├── aisec-plugin-host/      # Plugin manager, sandbox, permissions
│   └── aisec-storage/          # SQLite + sqlx + repositories
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
│   │   ├── aisec-plugin.toml
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
| `aisec-core` | `crates/aisec-core` | Shared error + logging frameworks |
| `aisec-storage` | `crates/aisec-storage` | SQLite persistence, migrations, repositories |
| `aisec-discovery` | `crates/aisec-discovery` | Attack-surface discovery engine (crawl, API, GraphQL, OpenAPI, AI) |
| `aisec-core` | `crates/aisec-core` | Shared errors, logging |
| `aisec-attack` | `crates/aisec-attack` | AI security attack framework |
| `aisec-payload` | `crates/aisec-payload` | Payload library, mutations, generation pipeline |
| `aisec-models` | `crates/aisec-models` | Local GGUF model manager, llama.cpp runtime |
| `aisec-judge` | `crates/aisec-judge` | AI judge engine — rule, regex, LLM consensus |
| `aisec-report` | `crates/aisec-report` | Report generation — HTML, PDF, JSON, SARIF |
| `aisec-fingerprint` | `crates/aisec-fingerprint` | AI endpoint provider fingerprinting |
| `aisec-plugin-host` | `crates/aisec-plugin-host` | Plugin lifecycle, sandbox, permissions |
| `aisec-auth` | `crates/aisec-auth` | Authentication engine (Playwright sessions) |
| `aisec-desktop` | `src-tauri` | Tauri application shell |
| `aisec-integration-tests` | `tests/integration` | Cross-crate smoke tests |

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

- **Rust:** `tracing` + `tracing-subscriber` + `tracing-appender` via `aisec-core::logging`
- **Frontend:** `createLogger()` in `src/shared/logging` with `VITE_LOG_LEVEL` support

### Error Handling

- **Rust:** `AisecError` / `ErrorCode` in `aisec-core`; `CommandError` IPC envelope in `src-tauri`
- **Frontend:** `AppError` type, `toAppError()`, and `ErrorBoundary` in `src/shared/errors`

## Next Steps (Not Implemented)

Future crates and modules described in `docs/ARCHITECTURE.md`:

- Security engines (`aisec-engine-*`)
- Orchestrator, storage, vault, plugin host
- Feature modules under `src/features/`
