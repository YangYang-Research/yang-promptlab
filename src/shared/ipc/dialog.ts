import { open } from "@tauri-apps/plugin-dialog";

/** Native OS file picker for SARIF finding imports. */
export async function pickSarifImportFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [
      { name: "SARIF", extensions: ["sarif", "json"] },
    ],
  });

  if (selected === null) {
    return null;
  }
  if (Array.isArray(selected)) {
    return selected[0] ?? null;
  }
  return selected;
}
