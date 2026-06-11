# Discovery Engine Status

**Crate:** `aisec-discovery` v0.1.0  
**Date:** 2026-06-10  
**Classification:** **Partial implementation**

---

## Verdict

| Classification | Applies? | Rationale |
|----------------|----------|-----------|
| **1. Real implementation** | Partially | Core crawl, HTTP, and detector logic is functional code — not stubs |
| **2. Partial implementation** | **Yes** | Works end-to-end as a library with known bugs and scope gaps |
| **3. Skeleton** | No | 16 source modules, BFS crawler, 4 detector families, integration tests |

`aisec-discovery` is a **working library MVP**, not a skeleton. It is **not production-complete** due to a critical crawler concurrency bug, limited JS analysis, no OpenAPI path expansion, and zero desktop-app integration.

---

## Capability Verification

### Summary matrix

| Capability | Status | Confidence | Evidence |
|------------|--------|------------|----------|
| HTTP crawling | **Partial** | High | Real reqwest + BFS; deadlock at `worker_count > 1` |
| JS parsing | **Partial** | High | Regex hints only; no AST; inline HTML scripts skipped |
| Link extraction | **Real** | High | `scraper` DOM parsing; unit tests pass |
| OpenAPI discovery | **Partial** | High | Spec URL detection works; no `paths` expansion |
| GraphQL discovery | **Real** | Medium–High | Introspection POST + playground heuristics verified |

---

## 1. HTTP Crawling

**Status: Partial implementation**

### What exists (real)

| Component | File | Behavior |
|-----------|------|----------|
| HTTP client | `client.rs` | `reqwest` with timeout, 2 MB body cap, 5 redirect limit, exponential retry |
| GET / POST | `client.rs` | `get()`, `post_json()` → `HttpSnapshot` |
| BFS crawler | `crawler.rs` | Concurrent workers, `max_depth`, `max_pages`, visited-set dedup |
| URL policy | `url_policy.rs` | Scheme check, localhost/private block (configurable), same-origin filter |
| Engine orchestration | `engine.rs` | Static probes first, then crawl; merges into `DiscoveryReport` |
| Config | `config.rs` | Depth, pages, workers, timeout, retry, SSRF toggle |

### Verified behavior

- **`http://localhost:3000`** with `allow_private_network: true`, `worker_count: 1`:
  - 4 pages fetched, 3 links extracted, 194 ms total
  - Seed → `/docs`, `/api/v1/users`, `/internal` crawled via extracted links

### Gaps / defects

| Issue | Severity | Detail |
|-------|----------|--------|
| **Worker deadlock** | Critical | `worker_count > 1` hangs indefinitely (`crawler.rs` notify pattern) |
| Default config unusable | High | Default `worker_count: 8` triggers deadlock |
| No robots.txt / sitemap | Medium | Not implemented |
| No auth headers / cookies | Medium | Unauthenticated crawl only |
| SSRF policy weak | Medium | Hostname literal check; no DNS resolve or redirect re-validation |
| No cancellation | Low | No abort token |
| Probe errors discarded | Low | `ProbeOutput.errors` always empty |

### Classification rationale

Crawling is **real** HTTP BFS logic, not a stub. Marked **partial** because the default multi-worker path is broken and several production crawl features are absent.

---

## 2. JS Parsing

**Status: Partial implementation**

### What exists

| Function | File | Method |
|----------|------|--------|
| `extract_url_hints()` | `extract.rs` | Regex over raw text — patterns for `/api…`, `/v1/…`, `/graphql…`, full URLs |
| `process_hints()` | `crawler.rs` | Called on **non-HTML** successful responses only |

### What does NOT exist

- No JavaScript AST parser (no `deno_core`, `quick-js`, `boa`, etc.)
- No execution of `<script>` blocks
- No webpack/chunk route extraction
- No source-map following

### Inline script gap

For HTML pages (`is_html()` = true), `process_html()` calls **`extract_links()` only** — it does not run `extract_url_hints()` on page body.

Example from test target homepage:

```html
<script>fetch("/api/v1/chat/completions");</script>
```

This URL is **not** extracted during crawl (no `<a href>`, no `script src`). Static AI probes may still find `/v1/chat/completions` separately (with the GET-404-skips-POST bug).

### Unit test coverage

- `extracts_api_hints_from_scripts` — passes on **raw JS string** input
- Does **not** test inline `<script>` inside HTML documents

### Classification rationale

Regex hint extraction is a **minimal partial** approach, not real JS parsing. Adequate for JSON/API response bodies; inadequate for modern SPAs with inline or bundled JS.

---

## 3. Link Extraction

**Status: Real implementation**

### What exists

| Function | File | Selectors / behavior |
|----------|------|----------------------|
| `extract_links()` | `extract.rs` | `a[href]`, `link[href]`, `script[src]`, `iframe[src]`, `form[action]` |
| URL resolution | `url_policy.rs` | `normalize_url()` — absolute + relative join, fragment strip |
| Dedup | `extract.rs` | Per-page `HashSet` |
| Enqueue policy | `crawler.rs` | Same-origin → crawl queue; external → `EndpointKind::Link` record |
| Filters | `extract.rs` | Skips `#`, `javascript:`, non-http(s) schemes |

### Verified behavior

- Unit test `extracts_anchor_and_form_links` — **pass**
- Live crawl of `localhost:3000` — 3 links extracted from `/` (→ `/docs`, `/api/v1/users`, `/internal`)

### Limitations (not blockers for "real")

- No `meta refresh`, `data-*`, or CSS `url()` extraction
- No `<area href>`, `<base href>` handling
- External links recorded but not crawled when `same_origin_only: true` (by design)

### Classification rationale

DOM-based link extraction via `scraper` is **complete for its defined scope** — a real implementation, not a placeholder.

---

## 4. OpenAPI Discovery

**Status: Partial implementation**

### What exists

| Layer | File | Behavior |
|-------|------|----------|
| Static probes | `detectors/paths.rs` | 12 common paths (`/openapi.json`, `/swagger.json`, `/v3/api-docs`, etc.) |
| Probe runner | `detectors/openapi.rs` | `probe_openapi_paths()` — GET each path |
| JSON detection | `detectors/openapi.rs` | `is_openapi_json()` — checks `"openapi"` or `"swagger"` keys |
| YAML detection | `detectors/openapi.rs` | `is_openapi_yaml()` — substring `openapi:` / `swagger:` |
| Crawl-time detection | `detectors/mod.rs` | `detect_from_snapshot()` on any fetched page |

### Verified behavior

- Unit test `detects_openapi_json` — **pass**
- Live probe on `localhost:3000/openapi.json` — detected, confidence 0.95
- Integration test (wiremock) — expects `/openapi.json` in report (test hangs on deadlock unless `worker_count: 1`)

### What is NOT implemented

- **No parsing of `paths` object** — spec URL found, individual API routes not expanded
- No OpenAPI 3 `$ref` resolution
- No HTML `<link rel="openapi">` or Swagger UI config extraction
- YAML detection is heuristic (string match), not full YAML parse
- Failed probe paths silently skipped (no error recording)

### Classification rationale

OpenAPI **endpoint discovery** (finding the spec file) is **real**. OpenAPI **surface expansion** (enumerating operations) is **not implemented** → overall **partial**.

---

## 5. GraphQL Discovery

**Status: Real implementation**

### What exists

| Layer | File | Behavior |
|-------|------|----------|
| Static probes | `detectors/paths.rs` | 6 paths (`/graphql`, `/api/graphql`, `/gql`, etc.) |
| Introspection POST | `detectors/graphql.rs` | Standard `IntrospectionQuery` JSON body |
| JSON response check | `detectors/graphql.rs` | `data.__schema` pointer match |
| Playground HTML | `detectors/graphql.rs` | Markers: graphiql, apollo sandbox, altair |
| URL heuristic | `detectors/graphql.rs` | `/graphql` in path, non-404 → lower confidence endpoint |
| Probe flow | `detectors/graphql.rs` | POST first, fallback GET |

### Verified behavior

- Unit test `detects_introspection_json` — **pass**
- Live probe on `localhost:3000/graphql` POST — detected, confidence 0.95
- Duplicate entry at 0.70 from URL path heuristic (dedupe keeps both — different method keys)

### Limitations

- Single introspection query; no full schema download or type enumeration stored
- No GraphQL over GET (`?query=`) probing
- No WS/subscription endpoint detection
- Auth-required endpoints may return 401/403 — still recorded via URL heuristic at lower confidence

### Classification rationale

GraphQL discovery is **functionally real** for endpoint identification via introspection and UI markers. Schema analysis beyond endpoint registration is out of scope → minor partial aspect, overall **real** for MVP discovery purposes.

---

## Architecture Overview

```
DiscoveryEngine::discover(seed_url)
    │
    ├─ validate_target_url()
    │
    ├─ run_static_probes(origin)          [if probe_static_paths]
    │     ├─ probe_openapi_paths()   → 12 GETs
    │     ├─ probe_graphql_paths()   → 6 paths POST+GET
    │     └─ probe_ai_paths()        → 14 paths GET (+ POST if GET fails)
    │
    └─ Crawler::run(seed)
          ├─ HttpClient::get(url)
          ├─ detect_from_snapshot()  → openapi, graphql, api, ai
          ├─ extract_links()           → HTML DOM [REAL]
          └─ extract_url_hints()       → regex [PARTIAL, non-HTML only]
```

---

## Module Inventory

| Module | Lines (approx.) | Role | Maturity |
|--------|-----------------|------|----------|
| `engine.rs` | 155 | Orchestrator | Real |
| `crawler.rs` | 328 | BFS crawl | Partial (deadlock) |
| `client.rs` | 156 | HTTP | Real |
| `extract.rs` | 115 | Links + hints | Links real, hints partial |
| `url_policy.rs` | 128 | SSRF / normalize | Partial |
| `config.rs` | 95 | Settings | Real |
| `retry.rs` | 125 | Backoff | Real |
| `detectors/openapi.rs` | 94 | OpenAPI | Partial |
| `detectors/graphql.rs` | 118 | GraphQL | Real |
| `detectors/ai.rs` | 165 | AI endpoints | Partial (POST skip bug) |
| `detectors/api.rs` | 87 | REST paths | Real |
| `detectors/paths.rs` | 75 | Probe lists | Real |
| `types.rs` | 126 | Report model | Real |

**Total:** 16 Rust source files — not a skeleton.

---

## Test Status

| Suite | Result | Notes |
|-------|--------|-------|
| Unit tests (detectors, extract, url_policy, config, retry) | ✅ Pass | Isolated logic verified |
| `crawler_respects_max_depth` | ❌ Hangs | Deadlock with `worker_count: 2` |
| Integration `discovers_openapi_graphql_ai_and_crawled_links` | ❌ Hangs | Same deadlock |
| Integration `rejects_private_targets_by_default` | ✅ Pass | SSRF default |
| Live `localhost:3000` (example, `worker_count: 1`) | ✅ Pass | 5 endpoints, 194 ms |

---

## Known Bugs Affecting Verification

| Bug | Impacts | Workaround |
|-----|---------|------------|
| Crawler worker deadlock | HTTP crawl with default config | `worker_count: 1` |
| AI probe GET-404 skips POST | POST-only AI endpoints missed | Fix probe logic |
| localhost blocked by default | Local dev targets | `allow_private_network: true` |

See [DISCOVERY_VERIFICATION_REPORT.md](DISCOVERY_VERIFICATION_REPORT.md) for full reproduction log.

---

## Integration Status

| Layer | Status |
|-------|--------|
| Library API (`DiscoveryEngine::discover`) | ✅ Callable |
| Tauri IPC | ❌ Not wired |
| SQLite persistence | ❌ Not wired |
| UI Discovery page | ❌ Mock data only |
| Plugin host | ❌ Separate sample plugins |

---

## Comparison to Skeleton

A skeleton would have empty `todo!()` bodies, trait definitions only, or hardcoded fake reports. This crate has:

- Working HTTP I/O via reqwest
- Concurrent crawl queue with visited tracking
- Four detector families with probe lists
- Structured `DiscoveryReport` output
- 15+ unit tests with real assertions
- Live verification against `localhost:3000`

**Conclusion:** Not a skeleton.

---

## Comparison to Full / Real Implementation

Missing for "complete" production discovery:

| Feature | Status |
|---------|--------|
| Fix worker pool deadlock | ❌ |
| JavaScript AST / SPA route extraction | ❌ |
| Inline script URL mining in HTML | ❌ |
| OpenAPI `paths` expansion | ❌ |
| robots.txt / sitemap / rate limiting | ❌ |
| Authenticated crawl | ❌ |
| DNS/redirect SSRF hardening | ❌ |
| Progress callbacks / cancellation | ❌ |
| Persistence + app integration | ❌ |

---

## Final Classification

```
aisec-discovery
├── Overall ..................... PARTIAL IMPLEMENTATION
├── HTTP crawling ............... PARTIAL (real logic, concurrency bug)
├── JS parsing .................. PARTIAL (regex hints only)
├── Link extraction ............. REAL
├── OpenAPI discovery ........... PARTIAL (spec find yes, path expand no)
└── GraphQL discovery ........... REAL (endpoint ID via introspection)
```

**Recommendation:** Safe to use as MVP discovery library with `worker_count: 1` and `allow_private_network` for local targets. Fix crawler deadlock before shipping default config. Wire into Tauri `scan_run` per [MVP_GAP_ANALYSIS.md](MVP_GAP_ANALYSIS.md).

---

*Related: [DISCOVERY.md](DISCOVERY.md), [DISCOVERY_VERIFICATION_REPORT.md](DISCOVERY_VERIFICATION_REPORT.md), [STATUS.md](STATUS.md)*
