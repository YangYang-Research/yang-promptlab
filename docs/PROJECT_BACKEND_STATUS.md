# Project Backend — Existence Status

> Question: before implementing the UI refactor (`docs/UI_REFACTOR_PLAN.md`, task **B1** etc.), which of the
> following already exist? — (1) Project Repository, (2) Project DTOs, (3) Project Service, (4) SQLite persistence.
> Verified against `main` (full app; PR #18 merged). No code written.

## Verdict (TL;DR)

| # | Item | Status | Where |
|---|------|--------|-------|
| 1 | Project Repository | ✅ **Exists (complete CRUD)** | `crates/aisec-storage` |
| 2 | Project DTOs | ✅ **Exists** (domain + IPC) | `aisec-storage` models + `src-tauri/src/dto.rs` |
| 3 | Project Service | ✅ **Exists** as command-op layer (no separate "service" module) | `src-tauri/src/commands/domain.rs` |
| 4 | SQLite persistence | ✅ **Exists** | `migrations/001_initial_schema.sql` + SQLite repo |

**Net:** the project backend is essentially complete. The **only gap** relevant to the refactor is that the
repository's `update` method is **not yet exposed as a Tauri command/IPC** — exactly task **B1** (and IPC **I1**).
No new repository, model, persistence, or service module is required.

---

## 1. Project Repository — ✅ Exists (complete CRUD)

- **Trait:** `ProjectRepository` — `crates/aisec-storage/src/repositories/traits.rs`
  ```
  create(CreateProject) -> Project
  get(id) -> Project
  list() -> Vec<Project>
  update(id, UpdateProject) -> Project
  delete(id) -> ()
  ```
- **Impl:** `SqliteProjectRepository` — `crates/aisec-storage/src/repositories/sqlite/project.rs` (real SQL for all 5
  methods, incl. `update`). Accessed via `Database::repositories().projects()`.
- **Tested:** `project_crud` unit test (create → update → list → delete) in the same file.
- **Refactor impact:** B1 (`project_update` command) can call `update` directly — no repository work needed.

## 2. Project DTOs — ✅ Exists (domain + IPC)

- **Domain models** — `crates/aisec-storage/src/models.rs`:
  - `Project { id, name, description, created_at, updated_at }` (row model, `FromRow`)
  - `CreateProject { name, description }`
  - `UpdateProject { name?, description? }` (derives `Default`, ready for partial updates)
- **IPC DTO** — `src-tauri/src/dto.rs`: `ProjectDto { id, name, description, created_at, updated_at }` with
  `From<Project>` (timestamps → RFC 3339 strings) for the frontend boundary.
- **Refactor impact:** none for create/get/list/delete. For B1, `project_update` reuses `UpdateProject` +
  `ProjectDto` as-is.

## 3. Project Service — ✅ Exists (as the command-op layer)

There is **no dedicated `service` module**; the service responsibilities live in the **command layer**:

- `src-tauri/src/commands/domain.rs` — testable `*_op` functions (take `&AppState`) wrapped by `#[tauri::command]`s:
  - `project_create_op` / `project_create`
  - `project_list_op` / `project_list`
  - `project_get_op` / `project_get`
  - `project_delete_op` / `project_delete`
- These contain the business logic (map errors → `CommandError`, structured logging) and operate directly on the
  repository — functionally the "project service".
- **Registered commands** (`src-tauri/src/lib.rs`): `project_create`, `project_list`, `project_get`, `project_delete`.

**Gap → matches B1:** there is **no `project_update_op` / `project_update` command** even though the repository
supports `update`. The refactor must add: `project_update_op` + `#[tauri::command] project_update` + registration,
and the IPC wrapper `updateProject` (I1). This is the documented B1/I1 scope — nothing else missing here.

## 4. SQLite persistence — ✅ Exists

- **Schema:** `crates/aisec-storage/migrations/001_initial_schema.sql` →
  `CREATE TABLE IF NOT EXISTS projects (id, name, description, created_at, updated_at)`. Migrations are embedded
  (`sqlx::migrate!`) and applied on startup (`Database::connect_path`).
- **Runtime:** the desktop app opens SQLite at `app_data_dir/aisec.db` on startup and stores the DB/repositories in
  `AppState` (backend integration, PR #15). Project rows persist and survive restart (verified earlier end-to-end).
- **Refactor impact:** none — persistence is in place and used by every project command.

---

## Implications for the UI refactor

- **B1 (`project_update`)** is the **only** backend task needed for Projects, and it's a thin command over an
  existing repo method + existing DTOs. Estimate in the plan (1h) stands.
- **I1** adds the `updateProject` TS wrapper.
- **F3/F4** (Projects edit modal, detail page, create→`/scans/new` redirect) are pure frontend over existing +
  B1/I1 commands.
- Do **not** create a new repository, models, persistence layer, or a separate "service" module — they already exist.
