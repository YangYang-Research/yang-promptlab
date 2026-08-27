import { invokeCommand } from "./invoke";

export const APP_UPDATE_PROGRESS_EVENT = "app-update-progress";

export type UpdateCheckDto = {
  currentVersion: string;
  latestVersion?: string | null;
  platform: string;
  updateAvailable: boolean;
  applied: boolean;
  notes?: string | null;
  skippedReason?: string | null;
};

export type UpdateProgressDto = {
  phase: string;
  message: string;
  currentVersion: string;
  latestVersion?: string | null;
  downloadedBytes?: number | null;
  totalBytes?: number | null;
};

export function checkForUpdate(): Promise<UpdateCheckDto> {
  return invokeCommand<UpdateCheckDto>("updater_check");
}

export function applyUpdateIfAvailable(): Promise<UpdateCheckDto> {
  return invokeCommand<UpdateCheckDto>("updater_apply_if_available");
}

export function updateDownloadPercent(progress: UpdateProgressDto): number | null {
  if (progress.phase !== "downloading") return null;
  const total = progress.totalBytes ?? 0;
  const downloaded = progress.downloadedBytes ?? 0;
  if (total <= 0) return null;
  return Math.min(100, Math.max(0, (downloaded / total) * 100));
}
