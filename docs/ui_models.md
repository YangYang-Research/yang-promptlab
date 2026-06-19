# Models Page UI

AISec Desktop **Models** page — vault overview, native imports, download manager, and installed model inventory.

## Vault overview

Summary cards at the top of the page show live stats from the Rust backend:

| Card | Source | Description |
|------|--------|-------------|
| Installed models | `models_vault_stats` | Count of registered vault entries |
| Installed size | `models_vault_stats` | Sum of registered model `size_bytes` |
| Vault disk usage | `models_vault_stats` | Recursive byte size of the vault directory |
| Runtime status | `runtime_status` | Embedded llama.cpp supervisor health |

![Vault overview cards](./screenshots/models-vault-overview.svg)

## Native file picker

Import uses `@tauri-apps/plugin-dialog` (`dialog:default` capability) for cross-platform file selection:

- **Browse GGUF** — filter `*.gguf`
- **Browse ZIP** — filter `*.zip`

Selected paths are read-only in the UI; import still flows through `models_import_gguf` / `models_import_zip`.

![Import with native file picker](./screenshots/models-file-picker.svg)

### Platform support

| OS | Backend |
|----|---------|
| Windows | `IFileOpenDialog` via Tauri dialog plugin |
| macOS | `NSOpenPanel` via Tauri dialog plugin |
| Linux | GTK file chooser via Tauri dialog plugin |

## Download Manager

Active catalog downloads surface in a dedicated **Download Manager** card with:

- Progress bar and percent
- Downloaded / total bytes
- **Remaining** size
- **Speed** (rolling average, updated every 500ms in the download loop)
- **ETA** (derived from speed and remaining bytes)
- **Pause**, **Resume**, **Cancel** (existing coordinator controls)

Progress is polled via `models_download_status` every 750ms while a download is active. On page load, any in-flight download is restored automatically.

![Download manager](./screenshots/models-download-manager.svg)

### IPC fields (`ModelDownloadProgressDto`)

```ts
{
  catalogId: string;
  status: string;
  downloadedBytes: number;
  totalBytes: number | null;
  remainingBytes: number | null;
  percent: number | null;
  speedBytesPerSec: number | null;
  etaSeconds: number | null;
  resumed: boolean;
  destination: string;
}
```

## Installed models

Each installed entry shows human-readable **installed size** (`formatBytes` on `sizeBytes`) plus verify, test, judge selection, and remove actions.

## Related commands

| Command | Purpose |
|---------|---------|
| `models_vault_stats` | Vault summary for overview cards |
| `models_download_start` | Start catalog download |
| `models_download_pause` / `resume` / `cancel` | Download controls |
| `models_download_status` | Poll progress + finalize on complete |
| `runtime_status` | Runtime health for overview card |
| `models_import_gguf` / `models_import_zip` | Import after file picker |

## Files

| Area | Path |
|------|------|
| Page | `src/features/models/ModelsPage.tsx` |
| Download card | `src/features/models/DownloadManagerCard.tsx` |
| File picker | `src/shared/ipc/dialog.ts` |
| Format helpers | `src/shared/utils/format.ts` |
| Models IPC | `src/shared/ipc/models.ts` |
| Runtime IPC | `src/shared/ipc/runtime.ts` |
| Backend | `src-tauri/src/commands/models.rs` |
| Download engine | `crates/aisec-models/src/download/` |
