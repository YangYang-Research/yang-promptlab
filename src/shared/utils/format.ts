const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** Human-readable byte size (base 1024). */
export function formatBytes(bytes: number, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "—";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(digits)} ${BYTE_UNITS[unitIndex]}`;
}

/** Transfer speed in bytes per second. */
export function formatSpeed(bytesPerSec: number | null | undefined): string {
  if (bytesPerSec == null || !Number.isFinite(bytesPerSec) || bytesPerSec <= 0) {
    return "—";
  }
  return `${formatBytes(bytesPerSec)}/s`;
}

/** Countdown from seconds remaining. */
export function formatEta(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) {
    return "—";
  }
  if (seconds <= 0) {
    return "done";
  }
  if (seconds < 60) {
    return `${Math.ceil(seconds)}s`;
  }
  if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.ceil(seconds % 60);
    return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
  }
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}
