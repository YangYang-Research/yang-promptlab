import { useEffect, useRef, useState } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ScanProgressEvent } from "@/shared/ipc";

type ScanConsoleProps = {
  scanId: string;
};

function formatLine(event: ScanProgressEvent): string {
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
    parts.push(`(${event.payload})`);
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

export function ScanConsole({ scanId }: ScanConsoleProps) {
  const [lines, setLines] = useState<string[]>([]);
  const tailRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setLines([]);
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    void listen<ScanProgressEvent>("scan-progress", (payload) => {
      if (payload.payload.scanId !== scanId) return;
      setLines((prev) => [...prev, formatLine(payload.payload)]);
    }).then((fn) => {
      if (cancelled) {
        void fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      void unlisten?.();
    };
  }, [scanId]);

  useEffect(() => {
    tailRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [lines]);

  return (
    <div className="scan-console">
      <h4 className="scan-console__title">Live Scan Console</h4>
      <pre className="scan-console__body" aria-live="polite">
        {lines.length === 0 ? (
          <span className="scan-console__placeholder text-muted">Waiting for scan events…</span>
        ) : (
          lines.map((line, index) => (
            <div key={`${index}-${line}`} className="scan-console__line">
              {line}
            </div>
          ))
        )}
        <div ref={tailRef} />
      </pre>
    </div>
  );
}
