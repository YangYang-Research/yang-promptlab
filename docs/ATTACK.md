# Attack pipeline

**Last verified:** 2026-08-22

Wizard execute path:

```
Verified Target Profile → Yazg plan → review
  → generate payloads → harness → target → judge → findings → report
```

Agent mode retries a category until a judge hit or budget exhausts. Wizard SSOT: [ARCHITECTURE.md](ARCHITECTURE.md#scan-wizard).

---

## Planner

Production: `planner_generate_from_profile` → `YazgSupervisor::react_plan` (AI Runtime must be live) → `attack_planner_adjust` → `scan_start`.

Shared types in `promptlab-planner` (`AttackPlan`: profile, categories, `disabled_tests`, rationales). Wizard plans come from the Target Profile + Yazg, not from crawl fingerprints.

---

## Payloads

| Crate | Role |
|-------|------|
| `promptlab-payload` | Embedded `data/payloads.json` + encoding mutations |
| `promptlab-generator` | Plan → `AttackContext.generated_payloads` |

Mutations: unicode / base64 / hex / HTML encode + wrap. Scan policy: `mutator_settings`.

Wizard **payload strategy** (playbook, not a separate IPC): `deterministic` | `mutation` | `adaptive` (adaptive = mutation + response-adaptation on retry). Maps to generator `StaticPack` / `TemplateMutation`. Extra flags: context awareness, conversation memory, payload dedup, cross-category mutation, `variantsPerTest` (1–20), `maxTotalPayloads` (1–50).

Generator modes: **static pack** (catalog + `disabled_tests`), **template mutation**, **local LLM** (2–3 extra probes/category, fallback static). Lazy at step 5. Executor prefers generated set, else builtins.

**Canary:** `promptlab-core` mints `PROMPTLAB-<SUITE>-<PAYLOAD_ID>-<NONCE>` (`{{CANARY}}` placeholder). Attack crate stamps/preserves canaries through mutators; echo in the target response is a success signal.

Catalog prefixes: `pi-*`, `spe-*`, `ta-*`, `mp-*`, `cul-*`, `agh-*`, `rag-*`, plus `jailbreak` / `mcp_abuse`.

---

## Executor (`promptlab-attack`)

| Category | ID |
|----------|-----|
| Prompt Injection | `prompt_injection` |
| System Prompt Extraction | `system_prompt_extraction` |
| Jailbreak | `jailbreak` |
| RAG Leakage | `rag_leakage` |
| Memory Poisoning | `memory_poisoning` |
| Cross User Leakage | `cross_user_leakage` |
| Agent Goal Hijacking | `agent_goal_hijacking` |
| Tool Abuse | `tool_abuse` |
| MCP Abuse | `mcp_abuse` |

Lifecycle: Planning → Preparing → Executing → Evaluating → Collecting → Completed.

Desktop delivery is **harness**, not raw `HttpTransport`. Results → `attack_results` / `findings`.

Attack-time mutators (`/mutators`, `mutator_settings`): `base64_wrap`, `unicode_homoglyph`, `delimiter_injection`, `role_swap`, `chunk_split`, `json_escape`, `repeat_amplify`, `hex_wrap`, `html_wrap`, `rot13_wrap`, `leetspeak`, `reversed_text`, `token_split`, `markdown_code_fence`, `zero_width_dense`. Evaluation: success, confidence 0–1, severity, indicators.

**Catalog:** `attack_catalog_techniques` seeded at startup; UI `/attack-categories` + `attack_catalog_generate_prompt` (Yazg).

Scan jobs: `scan_pause` / `resume` / `stop`. Findings: `finding_rejudge`, `finding_import_sarif`. Post-scan: `scan_recommendations_generate`, `finding_recommendations_generate`, `project_summary_generate` (Yazg + cached in playbook). Project `health_score` 0–100 after a completed attack scan.

```bash
cargo test -p promptlab-attack
cargo test -p promptlab-payload
```

---

## Agentic loop (`promptlab-agent`)

```
Plan → Generate → Attack → Judge → Retry?
```

Stop: `vulnerable: true`, max attempts (default 5), or user cancel. Retry escalates generator: initial → `TemplateMutation` → `LocalLlm`.

Playbook: `agent_mode: true`, `max_agent_attempts`. `scan_start` then runs `run_agent_scan_job`. Host: `src-tauri` scan execution (`pick_scan_execution_agent`: sequential vs agentic, reflection, endpoint pacing after failures). Sub-agents vs tools: [YAZG.md](YAZG.md#sub-agents), [YAZG.md](YAZG.md#tools).

---

## Judge (`promptlab-judge`)

Rules + regex + optional LLM roles (judge / classifier / attacker) → consensus. Inference goes through the gateway / harness — [RUNTIME.md](RUNTIME.md).

| Mode | Path |
|------|------|
| Deterministic | Rules + regex |
| Local / remote LLM | Gateway / harness |
| Consensus | Deterministic + LLM |

Weights (`judge_role_weights`): 0.85 / 0.80 / 0.75 / 0.65. Threshold 0.55, floor 0.45. LLM JSON: `vulnerable`, `confidence`, `severity`, `category`, `rationale`, `indicators`.

```bash
cargo test -p promptlab-judge
```

---

## Reports (`promptlab-report`)

IPC: `report_generate` / `list` / `read` / `export` / `export_scan`. Dir: `~/.promptlab/workspaces/reports/` (`promptlab-{kind}-{scan_id}.{ext}`).

| Kind | Audience |
|------|----------|
| Executive / Technical / Compliance | Leadership / engineers / OWASP+NIST |

Formats: HTML, PDF, JSON, SARIF 2.1.0, **CSV** (UI export dropdown). SARIF: critical/high → `error`, medium → `warning`, low → `note`. Charts: risk gauge, severity bars, category breakdown. Compliance refs: OWASP LLM + NIST AI RMF on findings/catalog.

```bash
cargo test -p promptlab-report
```
