import { useEffect, useState } from "react";

import { getScanStatus, type ScanStatusDto } from "@/shared/ipc";

const POLL_MS = 2000;

export function useScanStatuses(scanIds: string[], enabled: boolean) {
  const [statuses, setStatuses] = useState<Map<string, ScanStatusDto>>(new Map());

  useEffect(() => {
    if (!enabled || scanIds.length === 0) {
      setStatuses(new Map());
      return;
    }

    let cancelled = false;

    async function poll() {
      try {
        const results = await Promise.all(scanIds.map((id) => getScanStatus(id)));
        if (cancelled) return;
        setStatuses(new Map(results.map((status) => [status.scan_id, status])));
      } catch {
        // Polling errors are non-fatal; the page keeps the last snapshot.
      }
    }

    void poll();
    const timer = window.setInterval(() => void poll(), POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [enabled, scanIds.join("|")]);

  return statuses;
}

export function mergeScanStatus(
  scanId: string,
  storeStatus: string,
  live: ScanStatusDto | undefined,
  findingsFallback: number,
): ScanStatusDto {
  if (live) {
    return {
      ...live,
      findings_count: Math.max(live.findings_count, findingsFallback),
    };
  }

  return {
    scan_id: scanId,
    status: storeStatus,
    progress_percent: storeStatus === "completed" ? 100 : 0,
    completed: 0,
    total: 0,
    findings_count: findingsFallback,
    current_endpoint: null,
    current_test: null,
    started_at: null,
  };
}
