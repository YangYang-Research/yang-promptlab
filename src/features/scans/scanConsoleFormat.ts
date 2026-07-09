import type { ScanProgressEvent } from "@/shared/ipc";

export function formatScanConsoleLine(event: ScanProgressEvent): string {
  const time = new Date(event.timestamp);
  const stamp = Number.isNaN(time.getTime())
    ? "--:--:--"
    : time.toLocaleTimeString(undefined, { hour12: false });

  const parts = [`[${stamp}]`, event.message];
  if (event.endpoint) {
    const path = (() => {
      try {
        return new URL(event.endpoint).pathname;
      } catch {
        return event.endpoint;
      }
    })();
    parts.push(`@ ${path}`);
  }
  if (event.payload) {
    parts.push(`\n    payload: ${event.payload}`);
  }
  if (event.statusCode != null) {
    const latency = event.latency != null ? ` ${event.latency}ms` : "";
    parts.push(`→ ${event.statusCode}${latency}`);
  }
  if (event.findingId) {
    parts.push(`[finding ${event.findingId.slice(0, 8)}]`);
  }
  return parts.join(" ");
}
