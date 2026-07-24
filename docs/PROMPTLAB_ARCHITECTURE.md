# PromptLab Architecture Refactor

> Breaking refactor: PromptLab → PromptLab. No backward compatibility. Storage unified under `~/.promptlab/`.

## 1. Application Architecture

```
React UI (Vite)
    ↓ IPC
Tauri Shell (promptlab-desktop)
    ↓
AppState
 ├── EnvironmentPaths (~/.promptlab/*)
 ├── EventBus → OCSF Logger → logs/*.log
 ├── SQLite (workspaces/promptlab.db)
 ├── Models (root/models)
 ├── Runtime (root/runtime)
 ├── Plugins (root/plugins)
 └── Inference config (config/ai_runtime_config.json)
```

## 2. Root Directory Layout

| Path | Purpose |
|------|---------|
| `~/.promptlab/` | Root directory (all assets) |
| `config/` | Global configuration (`environment.json`, `ai_runtime_config.json`, `plugins_state.json`) |
| `workspaces/` | Projects, scans, reports, exports, `promptlab.db`, `AuthSessions/` |
| `models/` | Local GGUF models, registry, credentials |
| `runtime/` | Embedded libllama metadata, hardware profile |
| `logs/` | OCSF JSON Lines logs |
| `plugins/` | Installed plugins |
| `cache/` | Temporary caches |
| `temp/` | Working files |
| `backups/` | Workspace backups |

Override root: `PROMPTLAB_ROOT` env var. Override DB: `PROMPTLAB_DB_PATH`.

## 3. Environment Architecture

- **Module:** `crates/promptlab-core/src/environment.rs`
- **Bootstrap:** `bootstrap_environment()` on startup — creates dirs, validates read/write
- **IPC:** `environment_get`, `environment_update`
- **UI:** Settings → **Environments** (replaces Paths)

Workspaces contain **only** project data. Models, runtime, plugins, logs, and global config are outside workspaces.

## 4. Logging Architecture

```
Feature modules
    ↓ publish()
EventBus (in-process mpsc)
    ↓
Logger thread (sole file writer)
    ↓
OCSF JSON formatter
    ↓
Category log files + app.log aggregate
```

- **Module:** `crates/promptlab-core/src/event_log.rs`
- **IPC:** `logs_list_files`, `logs_tail`, `logs_recent_events`, `logs_open_folder`
- **UI:** Settings → Troubleshooting (live viewer, filters, auto-refresh)

### Log files

`app.log`, `system.log`, `runtime.log`, `models.log`, `scan.log`, `planner.log`, `attack.log`, `harness.log`, `auth.log`, `plugins.log`, `ui.log`, `payload.log`, `judge.log`, `workspace.log`, `projects.log`, `settings.log`

Every category event is also written to `app.log`.

## 5. Event Bus Design

- `EventBus::publish(OcsfEvent)` — non-blocking send to logger thread
- `EventRing` — in-memory ring buffer (2000 events) for Troubleshooting UI
- `global_event_bus()` — optional global accessor after startup
- Secrets masked before write (`mask_secrets`, attribute key redaction)

## 6. OCSF Event Schema

```json
{
  "timestamp": "2026-06-13T12:00:00Z",
  "severity": "informational",
  "category": "application",
  "classUid": 1001,
  "className": "Application Activity",
  "activityId": 1,
  "activityName": "Application Started",
  "module": "promptlab-desktop",
  "component": "startup",
  "workspaceId": null,
  "projectId": null,
  "scanId": null,
  "message": "PromptLab backend starting",
  "attributes": {}
}
```

## 7. Files Created

| File | Purpose |
|------|---------|
| `crates/promptlab-core/src/environment.rs` | Root directory layout + validation |
| `crates/promptlab-core/src/event_log.rs` | Event bus + OCSF logger |
| `src-tauri/src/commands/environment.rs` | Environment + logs IPC |
| `src/shared/ipc/environment.ts` | Frontend IPC wrappers |
| `src/features/settings/EnvironmentsPanel.tsx` | Environments settings UI |
| `src/features/settings/TroubleshootingPanel.tsx` | Troubleshooting + live logs |
| `docs/PROMPTLAB_ARCHITECTURE.md` | This document |

## 8. Files Renamed / Moved (conceptual)

| Before | After |
|--------|-------|
| Tauri `app_data_dir` | `~/.promptlab/` |
| `promptlab.db` at data root | `workspaces/promptlab.db` |
| `ai_runtime_config.json` at data root | `config/ai_runtime_config.json` |
| `plugins_state.json` at data root | `config/plugins_state.json` |
| `AuthSessions/` at data root | `workspaces/AuthSessions/` |
| Settings → Paths | Settings → Environments |
| Storage keys `promptlab:*` | `promptlab:*` |
| Product name PromptLab | PromptLab |
| Bundle ID `yangyang.promptlab.app` | `com.promptlab.desktop` |

## 9. Files Removed / Obsolete

- Settings **Paths** tab (replaced by Environments)
- Tauri `app_data_dir` as storage root (no longer used for app data)
- Legacy `~/.promptlab` path defaults in UI

## 10. Remaining Technical Debt

- Internal Rust crates still named `promptlab-*` (workspace package rename deferred)
- Executable still `promptlab-desktop` binary name in Cargo.toml
- Not all feature modules publish OCSF events yet (only startup/settings wired)
- `tracing` still writes `promptlab-trace.log` alongside OCSF logs (dev diagnostics)
- Plugin env vars `PROMPTLAB_PLUGIN_*` in plugin-host sandbox (rename to `PROMPTLAB_*`)
- Keychain service name still `com.promptlab.app` in promptlab-auth
- Consolidated single SQL schema file not yet merged (migrations still incremental)
- Crash panic hook not yet wired to `publish_crash()`
- Export Logs ZIP not implemented (copy log folder path only)
