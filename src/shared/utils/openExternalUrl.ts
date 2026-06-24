import { openUrl } from "@tauri-apps/plugin-opener";

/** Open an https/http URL in the system browser (Tauri) or a new tab (browser dev). */
export async function openExternalUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
