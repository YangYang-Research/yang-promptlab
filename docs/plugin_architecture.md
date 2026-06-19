# AISec Plugin Architecture

This document describes the extensibility layer: harness registry wiring, plugin host integration, and how discovery / attack / judge engines consume plugins.

## Overview

AISec separates **target delivery** (harness) from **engine extensions** (plugins):

| Layer | Crate | Responsibility |
|-------|-------|----------------|
| Harness registry | `aisec-harness` | Route payloads to HTTP, OpenAI-compatible, or Playwright targets |
| Harness factory | `aisec-harness` | Resolve `HarnessKind` → registered `Harness` implementation |
| Plugin host | `aisec-plugin-host` | Discover, enable, sandbox, and invoke extension plugins |
| Desktop runtime | `src-tauri` | Shared `AppState`, IPC, engine hook integration |

## Harness pipeline (production)

```
TargetDescriptor
    → HarnessFactory (backed by HarnessRegistry)
        → Harness::execute
            → NormalizedResponse
                → AttackExecutor / JudgeEngine
```

### Registry-backed factory

`HarnessFactory` registers built-in providers at startup:

- `http` — REST / generic HTTP
- `openai` — OpenAI-compatible chat APIs
- `playwright` — browser chat (session-scoped, injected per attack runtime)

Resolution path:

1. `TargetDescriptor::preferred_harness()` selects a `HarnessKind`
2. `HarnessFactory::resolve_kind()` looks up `registry.get_kind(kind)`
3. Playwright harnesses are registered per-request when a browser session is active

The shared factory lives in `AppState` and is cloned into per-attack runtimes. Session-specific Playwright instances are added via `with_playwright()` without replacing built-ins.

### Backward compatibility

- Existing `HarnessFactory::new()`, `resolve()`, `execute()` APIs are unchanged
- `HarnessTransport::for_attack_target()` still creates a standalone factory for unit tests
- Built-in harness IDs (`http`, `openai`, `playwright`) are stable

## Plugin host

### Plugin types

| Type | Default hook | Engine integration |
|------|--------------|-------------------|
| `discovery` | `discover` | Merges suggested endpoints after core crawler |
| `attack` | `execute_attack` | Mutates payload before harness delivery |
| `judge` | `evaluate` | Supplemental verdict signals merged into judge output |
| `report` | `render_report` | Available for future report formatters |

### Lifecycle

```
Discovered → Installed → Enabled → Loaded → Active
                ↓
            Disabled (persisted in plugins_state.json)
```

Plugins are discovered from `{data_dir}/plugins/` by scanning for `aisec-plugin.toml`. On first launch, bundled samples from `plugins/samples/` are copied when the directory is empty.

### Sandbox

Plugins run as subprocesses (Python / Node) with JSON-lines IPC. Capabilities (`probe_mutate`, `http_request`, `finding_emit`, etc.) are declared in the manifest and enforced by `PermissionGuard`.

## Desktop integration

### AppState

- `harness_factory: HarnessFactory` — shared registry-backed factory
- `plugin_manager: Arc<Mutex<PluginManager>>` — discovered plugins + enable state

### Attack path

```
PluginAwareTransport
    → mutate_attack_payload() (enabled attack plugins)
    → HarnessTransport
        → HarnessFactory::execute
```

### Discovery path

After the core `DiscoveryEngine` run, enabled discovery plugins may append candidate endpoints (deduplicated by URL).

### Judge path

After `JudgeEngine::judge_normalized()`, enabled judge plugins may elevate confidence / mark vulnerable when plugin signals exceed the core verdict.

## IPC commands

| Command | Purpose |
|---------|---------|
| `plugins_list` | List installed plugins |
| `plugins_refresh` | Rescan directory + restore enabled state |
| `plugins_enable` | Enable plugin (persisted) |
| `plugins_disable` | Disable plugin (persisted) |
| `plugins_info` | Directory path + counts by type |

## UI

**System → Plugins** — lists installed plugins with enabled/disabled status and toggle actions.

## Related code

- `crates/aisec-harness/src/factory/harness_factory.rs`
- `crates/aisec-harness/src/registry/harness_registry.rs`
- `crates/aisec-plugin-host/src/manager.rs`
- `crates/aisec-plugin-host/src/integrations.rs`
- `src-tauri/src/plugin_service.rs`
- `src-tauri/src/plugin_transport.rs`
- `src-tauri/src/commands/plugins.rs`
- `plugins/samples/` — reference implementations

## Writing plugins

See `plugins/samples/README.md` and `docs/PLUGINS.md` for manifest schema and SDK packages (`packages/plugin-sdk-python`, `packages/plugin-sdk-js`).

Attack plugins mutate payloads; they do **not** replace harness delivery unless a future harness plugin type is added. This preserves compatibility with existing sample plugins and built-in HTTP/OpenAI/Playwright paths.
