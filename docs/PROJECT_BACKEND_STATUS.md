# Project Backend — Existence Status

> Assessment for the UI refactor (`docs/UI_REFACTOR_PLAN.md`). Question: which of the following already exist?
> (1) Project Repository, (2) Project DTOs, (3) Project Service, (4) SQLite persistence.
> Verified against the current codebase. No code written.

## Verdict (TL;DR)

| # | Item | Status | Where |
|---|------|--------|-------|
| 1 | Project Repository | ✅ **Exists (complete CRUD)** | `crates/aisec-storage` |
| 2 | Project DTOs | ✅ **Exists** (storage models + Rust/TS IPC DTOs) | `aisec-storage` + `src-tauri` + `src/shared/ipc` |
| 3 | Project Service | ⚠️ **Partial** — command-op layer, no dedicated service module; **update not exposed** | `src-tauri/src/commands/domain.rs` |
| 4 | SQLite persistence | ✅ **Exists** | `migrations/001_initial_schema.sql` + `Database` / `AppState` |

**Net:** repository, models, persistence, and four of five project operations are in place. The **only backend gap**
for Projects in the refactor is exposing `ProjectRepository::update` as `project_update` (**B1**) and the matching
IPC wrapper `updateProject` (**I1**). No new repository, schema, or service module is required.

---

## 1. Project Repository — ✅ Exists (complete CRUD)

- **Trait:** `ProjectRepository` — `crates/aisec-storage/src/repositories/traits.rs`
  - `create(CreateProject) -> Project`
  - `get(id) -> Project`
  - `list() -> Vec<Project>`
  - `update(id, UpdateProject) -> Project`
  - `delete(id) -> ()`
- **Implementation:** `SqliteProjectRepository` — `crates/aisec-storage/src/repositories/sqlite/project.rs`
  - Real SQL for all five methods, including partial `update` (merge optional fields, bump `updated_at`).
- **Factory:** `Repositories::projects()` — `crates/aisec-storage/src/repositories/sqlite/mod.rs`
- **Tests:** `project_crud` unit test in `project.rs` (create → update → list → delete).

**Refactor impact:** B1 (`project_update` command) calls `update` directly — no repository work needed.

---

## 2. Project DTOs — ✅ Exists

### Storage / domain models (`crates/aisec-storage/src/models.rs`)

| Type | Purpose |
|------|---------|
| `Project` | Row model (`FromRow`): `id`, `name`, `description`, `created_at`, `updated_at` |
| `CreateProject` | Input for `create`: `name`, `description?` |
| `UpdateProject` | Input for `update`: `name?`, `description?` (`Default` for partial patches) |

### Rust IPC DTO (`src-tauri/src/dto.rs`)

- `ProjectDto { id, name, description, created_at, updated_at }` with `From<Project>` (timestamps → RFC 3339 strings).

### TypeScript IPC DTO (`src/shared/ipc/client.ts`)

- `ProjectDto` type mirrors the Rust shape (`created_at` / `updated_at` as strings).
- Wrappers today: `listProjects`, `createProject`, `getProject`, `deleteProject` — **no `updateProject` yet** (I1).

**Refactor impact:** B1 reuses `UpdateProject` + `ProjectDto` as-is. I1 adds the missing TS wrapper only.

---

## 3. Project Service — ⚠️ Partial (command-op layer, not a separate module)

There is **no dedicated `ProjectService` module or crate**. Project behavior lives in the Tauri **command layer**:

| Layer | Location | Operations |
|-------|----------|------------|
| Testable ops | `src-tauri/src/commands/domain.rs` | `project_create_op`, `project_list_op`, `project_get_op`, `project_delete_op` |
| Tauri commands | same file | `project_create`, `project_list`, `project_get`, `project_delete` |
| Registration | `src-tauri/src/lib.rs` | all four commands registered in `invoke_handler` |

Each `*_op` function takes `&AppState`, maps repository errors to `CommandError`, logs, and returns `ProjectDto`.
This is the functional equivalent of a thin service layer used elsewhere in the app.

**Gap (matches B1 / I1):**

- `ProjectRepository::update` exists but **`project_update_op` / `project_update` are absent**.
- Frontend has no `updateProject` IPC call.

**Refactor impact:** add `project_update_op` + `#[tauri::command] project_update` + `lib.rs` registration, then
`updateProject` in `client.ts`. Do **not** introduce a new service module unless the team wants a structural refactor
beyond the plan.

---

## 4. SQLite persistence — ✅ Exists

### Schema

- `crates/aisec-storage/migrations/001_initial_schema.sql`:
  ```sql
  CREATE TABLE IF NOT EXISTS projects (
      id          TEXT PRIMARY KEY NOT NULL,
      name        TEXT NOT NULL,
      description TEXT,
      created_at  TEXT NOT NULL,
      updated_at  TEXT NOT NULL
  );
  ```
- Child tables (`targets`, `scans`, `findings`, …) reference `projects(id)` with `ON DELETE CASCADE`.

### Runtime

- `Database::connect` / `Database::connect_path` — `crates/aisec-storage/src/pool.rs`
  - Embeds migrations via `sqlx::migrate!("./migrations")`, applies on connect.
  - WAL journal (with TRUNCATE fallback), foreign keys enabled.
- Desktop app opens `<app_data_dir>/aisec.db` — `src-tauri/src/db.rs` (`DB_FILENAME = "aisec.db"`).
- `AppState` holds `Database` and exposes `repositories()` — `src-tauri/src/state.rs`.

### Tests

- Repository: `project_crud` in `sqlite/project.rs`.
- Integration: `src-tauri/tests/project_commands.rs` (create/list/get/delete + reopen-from-disk).
- Crate integration: `tests/integration/tests/storage_persistence.rs` (file-backed survive reconnect).

**Refactor impact:** none — persistence is live and used by all existing project commands.

---

## Mapping to `docs/UI_REFACTOR_PLAN.md`

| Plan task | Depends on | Status |
|-----------|------------|--------|
| **B1** — `project_update` command | `ProjectRepository::update`, `UpdateProject`, `ProjectDto` | Repo + DTOs ready; command missing |
| **I1** — `updateProject` IPC | B1 | TS `ProjectDto` exists; wrapper missing |
| **F3** — Projects CRUD + create→wizard | I1 | Edit flow blocked until B1/I1 |
| **F4** — Project detail page | existing list/get IPC | Can load project by id today |

The plan’s note on B1 is accurate: *“Expose `ProjectRepository::update` (already exists) as a Tauri command + DTO
passthrough.”*

---

## Recommendations

1. **Do not rebuild** repository, storage models, SQLite schema, or a new service crate — they exist.
2. **Implement B1 then I1** as thin passthroughs; add an integration test mirroring `project_commands.rs` patterns.
3. **F3 edit modal** can proceed immediately after I1 lands; create/delete/get/list already work end-to-end.
