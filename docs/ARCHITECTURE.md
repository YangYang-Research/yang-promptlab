# Architecture

**Last verified:** 2026-08-23

PromptLab is an offline-first Tauri 2 desktop app for authorized AI security testing (LLM apps, chatbots, agents, MCP, RAG). Mapped to OWASP LLM / Agentic / MCP and NIST AI RMF.

| Principle | Implementation |
|-----------|----------------|
| Offline-capable | SQLite + remote HTTP AI providers (incl. custom OpenAI-compatible) |
| Local data sovereignty | All state under `~/.promptlab/` (`PROMPTLAB_ROOT`) |
| Extensibility | Harness providers |
| Auditability | Scan console, AgentTrace SQLite, structured logs |

Platforms: **Windows**, **macOS**, **Linux**. Product AI is **remote-only** (no embedded llama.cpp). Local servers such as Ollama are added as a custom OpenAI-compatible endpoint (`baseUrl` + model id).

---

## Layers

```
React UI (HashRouter)  src/features/* → src/shared/ipc
        │ invoke / listen
promptlab-desktop (src-tauri)  commands/* → AppState → crates
        │
   storage (SQLite)   harness (AI I/O)   inference (gateway)   runtime (remote host)
```

Browser-only `npm run dev` has **no IPC** — empty workspace, not mock fixtures.

Routes: `/`, `/projects`, `/scans`, `/scans/new`, `/targets`, `/findings`, `/reports`, `/runtime`, `/yazg`, `/models`, `/attack-categories`, `/mutators`, `/agent-trace`, `/settings`. Leftover route: `/plugins`.

First-run **Getting started** checklist (dashboard): runtime mode → add model → load model → first project/scan.

**Settings:** General, AI Runtime, Usage (token counters), Network (HTTP/SOCKS proxy + `allow_insecure_tls`), Data & storage (`environment_*`, `app_clear_all_data`), Diagnostics (OCSF logs, `db_health`, `security_audit` / `security_migrate_secrets`), About.

**Attack Categories** (`/attack-categories`): SQLite `attack_catalog_techniques` — enable/edit/reset prompts; `attack_catalog_generate_prompt` uses Yazg. **Mutators** (`/mutators`): global + per-category allowlist (`mutator_settings`).

---

## Repository

```
yang-promptlab/
├── src/                        # React UI
├── src-tauri/                  # promptlab-desktop
├── crates/                     # engines (table below)
├── packages/plugin-sdk-{python,js}/  # leftover
├── plugins/                    # leftover samples
├── resources/                  # optional bundled assets
├── runtime/                    # legacy notes
└── tests/{frontend,integration}/
```

```bash
npm install
npm run dev              # UI only
npm run tauri dev        # desktop + IPC
npm run build
npm test
cargo test -p promptlab-core
```

| Concern | Location |
|---------|----------|
| Logging (Rust / UI) | `promptlab-core` + `tracing` / `src/shared/logging` |
| Structured events | OCSF-shaped JSONL via `EventBus` (`logs_recent_events`, Settings → Diagnostics) |
| Errors | `PromptLabError` → `CommandError` / `src/shared/errors` |
| Outbound HTTP | `promptlab-core::proxy` (`proxy_get/set/test_connection`) |

---

## Crates

| Crate | Role |
|-------|------|
| `promptlab-core` | Errors, `~/.promptlab` layout, logging |
| `promptlab-storage` | SQLite + repositories |
| `promptlab-harness` | Normalized AI I/O |
| `promptlab-inference` | Gateway, token usage, traffic |
| `promptlab-runtime` | Remote-oriented runtime host |
| `promptlab-models` | Model vault / third-party registry |
| `promptlab-target-profile` | Wizard target SSOT + verify |
| `promptlab-planner` | Attack-plan types |
| `promptlab-payload` / `promptlab-generator` | Catalog + plan → probes |
| `promptlab-attack` / `promptlab-judge` | Execute / verdict |
| `promptlab-agent` / `promptlab-agenttrace` | Yazg supervisor + sub-agents, spans |
| `promptlab-report` | HTML / PDF / JSON / SARIF / CSV |
| `promptlab-auth` | Keychain + descriptor secret hydrate |
| `promptlab-plugin-host` | **Unused by product** (leftover sandbox crate) |
| `promptlab-desktop` | `src-tauri` |
| `promptlab-integration-tests` | `tests/integration` |
| `promptlab-discovery` / `promptlab-fingerprint` / `promptlab-endpoint-metadata` | **Unused by desktop** (crawl-era libraries) |

---

## Data root (`~/.promptlab/`)

```
config/          environment.json, ai_runtime_config.json, plugins_state.json
workspaces/      promptlab.db, reports/, AuthSessions/*.storage.enc
models/          legacy path (registry is SQLite; no local weight vault)
runtime/         (legacy local-runtime dir; hardware profile is in SQLite)
logs/  plugins/  cache/  temp/  backups/
agenttrace/agenttrace.db
```

Not Tauri `app_data_dir`. Secrets: OS keychain `com.promptlab.app` — see [AUTH.md](AUTH.md).

### SQLite (`promptlab-storage`)

WAL, `PRAGMA foreign_keys = ON`. Schema: `crates/promptlab-storage/migrations/001_initial_schema.sql`.

Model registry rows live in SQLite (`models`). AgentTrace is a separate DB.

| Table | Notes |
|-------|-------|
| `projects` | `summary_json`, `health_score` |
| `targets` | `descriptor_json` (sanitized), `profile_json` (SSOT) |
| `scans` | `playbook_json` |
| `findings` | FTS5 `findings_fts` |
| `payloads` / `attack_results` / `reports` / `plugins` | |
| `endpoints` | Leftover crawl-era rows; wizard scans do not populate this |
| `attack_catalog_techniques` | Seeded catalog |
| `auth_profiles` / `auth_sessions` / `auth_recordings` | keychain refs |
| `runtime_traffic_*` / `judge_role_weights` / `mutator_settings` | |
| `agent_short_term_memory` / `agent_long_term_memory` | |

IDs are UUID `TEXT`; timestamps RFC 3339 UTC. Access: `Database::connect` → `db.repositories()`. Model registry: `models` table via `ModelRepository`.

---

## Harness (AI I/O)

Every completion: `HarnessFactory::execute` + `HarnessPurpose`. Feature crates must not open their own HTTP clients.

| Purpose | Caller |
|---------|--------|
| `attack` / `verify` | Scan / wizard |
| `assistant` / `judge` / `wizard` / `planner` / `generator` / `report` / `health` | Product inference |

```
Caller → HarnessFactory::execute
  → purpose policy (token caps)
  → interceptors
  → Harness (http | openai | anthropic | gemini | bedrock | llama | dify | mcp | websocket)
  → NormalizedResponse
```

Target-descriptor auth is for attack/verify; vault credentials are `AuthMaterial` for assistant/judge.

Add a provider: implement `Harness` under `crates/promptlab-harness/src/providers/`, `registry.register`. Details: [RUNTIME.md](RUNTIME.md).

---

## Scan wizard

SSOT is **Target Profile** (`targets.profile_json`). There is no crawl step.

```
Project → AI Target Profile → Auth / Verify
  → Yazg attack plan → Review → Execute → Results
```

Execute: `Approved plan → generator → harness → target → judge → findings`.

`TargetProfile`: provider, method, URL, headers, `{{PROMPT}}` template, capabilities, verified flag. Auth stays in `descriptor_json`. Verify is connect (`Hello`) then capability probe then Yazg classify — [DISCOVERY.md](DISCOVERY.md).

| Step | IPC |
|------|-----|
| Verify | `target_profile_verify*` (connect = harness only; classify needs AI Runtime) |
| Plan | `planner_generate_from_profile` (needs AI Runtime live) |
| Adjust | `attack_planner_adjust` |
| Run | `scan_start` — payloads lazy at step 5 |

Playbook stores `target_profile: true` (not `endpoint_ids`). Wizard extras: **Import API** (cURL → profile), drafts (`scan_wizard_save/load`). Jobs: `scan_pause` / `resume` / `stop`; interrupted scans reconciled on startup/shutdown.

Pipeline: [ATTACK.md](ATTACK.md). Yazg: [YAZG.md](YAZG.md).

---

## IPC

Handlers in `src-tauri/src/lib.rs`. Clients: `src/shared/ipc/`.

**Events:** `scan-progress`, `app-data-changed`, `runtime-install-progress`.

| Group | Commands |
|-------|----------|
| App | `health`, `app_info`, `app_clear_all_data`, `db_health`, `environment_*`, `proxy_*`, `logs_*`, `security_*` |
| CRUD | `project_*`, `target_*`, `scan_create/list/get/delete`, `finding_*`, `report_*` |
| Scan job | `scan_start/status/pause/resume/stop`, `scan_console_tail`, `scan_wizard_*`, `*_recommendations_generate`, `project_summary_generate` |
| Profile | `target_profile_*`, `planner_generate_from_profile`, `attack_planner_adjust` |
| Auth | (credential path via target/profile commands; see [AUTH.md](AUTH.md)) |
| Models / runtime | `models_*`, `runtime_*`, `mutator_settings_*` |
| Yazg | `yazg_*`, `agenttrace_*`, `agent_memory_*` |
| Catalog | `attack_catalog_*` |
