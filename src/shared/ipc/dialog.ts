import { open } from "@tauri-apps/plugin-dialog";

export type ModelImportKind = "gguf" | "zip";

const IMPORT_FILTERS: Record<ModelImportKind, { name: string; extensions: string[] }> = {
  gguf: { name: "GGUF Model", extensions: ["gguf"] },
  zip: { name: "ZIP Package", extensions: ["zip"] },
};

/** Native OS file picker for GGUF or ZIP imports (Windows, macOS, Linux). */
export async function pickModelImportFile(kind: ModelImportKind): Promise<string | null> {
  const filter = IMPORT_FILTERS[kind];
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [filter],
  });

  if (selected === null) {
    return null;
  }
  if (Array.isArray(selected)) {
    return selected[0] ?? null;
  }
  return selected;
}

/** Native OS file picker accepting either a GGUF model or a ZIP package. */
export async function pickAnyModelImportFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Model file (GGUF or ZIP)", extensions: ["gguf", "zip"] }],
  });

  if (selected === null) {
    return null;
  }
  if (Array.isArray(selected)) {
    return selected[0] ?? null;
  }
  return selected;
}

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
