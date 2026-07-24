# Báo cáo: Vì sao chưa thao tác được trên ứng dụng & app vẫn là mockup

> **Trạng thái (2026-06-12):** Tài liệu lịch sử. Tích hợp desktop đã hoàn tất qua PR #19 — app
> dùng IPC + SQLite, không còn mock store. Giữ lại để tham chiếu nguyên nhân gốc.

**Ngày:** 2026-06-12
**Phạm vi:** Giao diện desktop (React UI + Tauri shell) trong `src/` và `src-tauri/`.
**Kết luận ngắn:** Ứng dụng desktop hiện vẫn là **bản vỏ chạy dữ liệu giả (mock)**. Mọi nút hành
động (ví dụ "New Project") **chưa được nối** với backend, nên bấm vào không tạo/không lưu gì cả.
Phần "xương sống tích hợp" UI ↔ Tauri IPC ↔ database (các mục B1/B2/B3 trong
`docs/MVP_EXECUTION_PLAN.md`) vẫn chưa được làm.

---

## 1. Tóm tắt nguyên nhân

| # | Nguyên nhân | Bằng chứng |
|---|-------------|------------|
| C1 | Dữ liệu hiển thị nạp từ **mock**, không lấy từ DB | `src/app/store/AppStore.tsx:23-31` seed từ `@/shared/mock/data` |
| C2 | Các **nút hành động không có `onClick`** (nút chết) | `src/features/**` — không có `onClick` nào cho nút hành động (xem §6) |
| C3 | Lớp **IPC chỉ có 2 lệnh** bootstrap | `src/shared/ipc/client.ts:43-49` chỉ có `health`, `app_info` |
| C4 | **Backend Tauri chỉ có 2 command**, không mở DB | `src-tauri/src/commands/mod.rs` chỉ có `health`, `app_info` |
| C5 | `AppState` **không chứa database** | `src-tauri/src/state.rs` chỉ giữ `_log_guard` |
| C6 | Tauri **không phụ thuộc các crate domain** | `src-tauri/Cargo.toml:21-26` chỉ có `promptlab-core` (không có `promptlab-storage`) |
| C7 | Store **không có action tạo/persist** dữ liệu | `src/app/store/AppStore.tsx:50-90` reducer chỉ có các action UI |

Hệ quả: dù backend (storage, discovery, attack, judge, report) đã chạy được ở tầng thư viện, **không
có đường dẫn nào** để giao diện gọi xuống. App chỉ thao tác trên mảng dữ liệu giả trong bộ nhớ.

---

## 2. Truy vết cụ thể: bấm "New Project" thì điều gì xảy ra?

1. Nút được khai báo **không có handler**:
   ```tsx
   // src/features/projects/ProjectsPage.tsx:83
   <Button variant="primary">New Project</Button>
   ```
   Component `Button` có hỗ trợ `onClick` (`src/shared/components/Button.tsx:25`), nhưng trang
   không truyền vào → **bấm không kích hoạt gì**.
2. Kể cả nếu có handler, **chưa có hàm IPC** `createProject` để gọi (`src/shared/ipc/index.ts` chỉ
   export `getAppInfo`, `healthCheck`).
3. Kể cả nếu gọi IPC, **backend chưa có command** `project_create` (`src-tauri/src/commands/mod.rs`).
4. Kể cả nếu có command, **backend chưa mở SQLite** (`AppState` không có `Database`;
   `src-tauri/Cargo.toml` không phụ thuộc `promptlab-storage`).
5. Danh sách project hiển thị đến từ `mockProjects`, và reducer **không có** action thêm project
   (`AppStore.tsx`), nên cũng không có chỗ để hiển thị project mới.

→ Đứt ở **cả 5 mắt xích**, nên thao tác tạo project không thể hoạt động trên app.

---

## 3. "Connected" trên app không có nghĩa là dữ liệu thật

Khi chạy bằng Tauri, app gọi `health`/`app_info` thành công và hiện trạng thái **"Connected"**
(`src/App.tsx`). Nhưng đó chỉ là kiểm tra kết nối IPC bootstrap — **dữ liệu domain vẫn là mock**.
Cờ `backendConnected` không hề đổi nguồn dữ liệu (vẫn là `mockProjects`...). Đây là điểm dễ gây hiểu
lầm "app đã chạy thật".

---

## 4. Vì sao bài End-to-End Validation vẫn PASS?

Bài validate (`tests/integration/tests/mvp_flow.rs`, xem `docs/MVP_VALIDATION_REPORT.md`) **đi vòng
qua giao diện**: nó gọi **thẳng API thư viện Rust**, không qua nút bấm hay IPC. Ví dụ bước tạo
project trong harness:

```rust
let project = repos.projects().create(CreateProject { ... }).await; // gọi thẳng SQLite, không qua UI/IPC
```

Vì vậy:
- **PASS** = chứng minh các "động cơ" backend ghép lại chạy đúng (ở tầng thư viện).
- **Không** chứng minh app bấm được, vì harness cố tình không chạm tới lớp UI/IPC — đúng phần đang
  thiếu (C2–C6).

Hai kết quả không mâu thuẫn: chúng kiểm thử **hai tầng khác nhau**.

| | Validate MVP (PASS) | App desktop (chưa thao tác được) |
|---|---|---|
| Tầng | Thư viện Rust (gọi API crate) | UI → Tauri IPC → backend |
| Tạo project bằng | `repos.projects().create()` | bấm nút "New Project" |
| Qua UI/IPC? | Không | Có (nhưng chưa nối) |

---

## 5. Bằng chứng (file · dòng)

- `src/app/store/AppStore.tsx:11-19, 23-31` — import và seed state từ `mockProjects`, `mockTargets`, …
- `src/app/store/AppStore.tsx:50-90` — reducer chỉ có action UI (`SET_SEARCH`, `TOGGLE_SIDEBAR`,
  `SET_SELECTED_PROJECT`, `SET_SEVERITY_FILTER`, `UPDATE_FINDING_STATUS`, `UPDATE_SETTING`); **không**
  có create/persist.
- `src/shared/mock/data.ts` — nguồn dữ liệu giả (`mockProjects`, `mockTargets`, …).
- `src/shared/ipc/client.ts:43-49` — chỉ `healthCheck()` và `getAppInfo()`.
- `src/shared/ipc/index.ts` — chỉ export `getAppInfo`, `healthCheck`.
- `src-tauri/src/commands/mod.rs` — chỉ `health` và `app_info`.
- `src-tauri/src/state.rs:4-6` — `AppState` chỉ chứa `_log_guard` (không có `Database`).
- `src-tauri/Cargo.toml:21-26` — chỉ phụ thuộc `promptlab-core` (thiếu `promptlab-storage`, `promptlab-discovery`,
  `promptlab-attack`, `promptlab-judge`, `promptlab-report`).
- Quét toàn bộ `src/`: chỉ có `onClick` ở bộ lọc severity (`FindingsPage.tsx:98,107`) và nút thu gọn
  sidebar (`Sidebar.tsx:77`). **Không có** `onClick` cho bất kỳ nút hành động nào.

---

## 6. Danh sách nút hành động "chết" theo trang (không có handler)

| Trang | Nút | Vị trí |
|-------|-----|--------|
| Dashboard | New Project | `DashboardPage.tsx:33` |
| Projects | Import, New Project | `ProjectsPage.tsx:82-83` |
| Targets | Import OpenAPI, Add Target | `TargetsPage.tsx:88-89` |
| Discovery | Configure Modules, Start Scan, Pause, View Results, Run Now | `DiscoveryPage.tsx:20,21,59,61,64` |
| Attacks | Playbook, Launch Attack, Configure (×9) | `AttacksPage.tsx:77,78,90` |
| Findings | Export SARIF, Triage | `FindingsPage.tsx:88,89` |
| Reports | Download, Templates, Generate Report | `ReportsPage.tsx:69,83,84` |
| Models | Browse HuggingFace, Download Model, Verify, Remove, Download, Cancel | `ModelsPage.tsx:20,21,54,55,59,62` |
| Settings | View Logs | `SettingsPage.tsx:124` |

Ngoài ra, các thay đổi đang có (đổi status finding, đổi settings) chỉ nằm trong bộ nhớ phiên làm việc,
**không lưu** — refresh là mất.

---

## 7. Cần làm gì để app thao tác được thật (chưa thực hiện)

Đây là phần tích hợp UI ↔ Tauri ↔ DB (tương ứng B1/B2/B3 trong `docs/MVP_EXECUTION_PLAN.md`):

1. **Backend Tauri (B1):**
   - Thêm `promptlab-storage` (và các crate engine khi cần) vào `src-tauri/Cargo.toml`.
   - Mở SQLite và lưu `Database` trong `AppState` lúc khởi động.
2. **Lệnh IPC domain (B2):** thêm `#[tauri::command]` cho `project_create`, `project_list`,
   `target_create`, `scan_run`, `findings_list`, `report_generate`.
3. **Nối giao diện (B3):**
   - Bổ sung `createProject()/listProjects()/…` trong `src/shared/ipc/`.
   - Gắn `onClick` cho các nút (mở form "New Project", gọi IPC, nạp lại danh sách từ DB).
   - Bỏ/giấu `src/shared/mock/data.ts`; nạp store từ IPC thay vì mock; đổi nguồn dữ liệu theo
     `backendConnected`.

Sau khi làm xong 3 bước trên, nút "New Project" sẽ tạo và lưu project thật vào SQLite qua ứng dụng.

---

## 8. Ghi chú

- Báo cáo này chỉ mô tả hiện trạng; **không** thay đổi mã nguồn.
- Backend đã sẵn sàng (đã được kiểm chứng end-to-end ở tầng thư viện), nên khối lượng còn lại chủ yếu
  là **lắp ghép** (IPC + handler UI), không phải viết lại engine.
