import { useCallback, useEffect, useRef, useState } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { tailScanConsole, type ScanProgressEvent } from "@/shared/ipc";

type ScanConsoleProps = {
  scanId: string;
};

const TAIL_POLL_MS = 1_000;

export function ScanConsole({ scanId }: ScanConsoleProps) {
  const [content, setContent] = useState("");
  const bodyRef = useRef<HTMLPreElement>(null);
  const offsetRef = useRef(0);
  const tailingRef = useRef(true);

  const appendTail = useCallback(async () => {
    if (!tailingRef.current) return;
    try {
      const tail = await tailScanConsole(scanId, offsetRef.current);
      if (!tailingRef.current) return;
      offsetRef.current = tail.offset;
      if (tail.content) {
        setContent((prev) => prev + tail.content);
      }
    } catch {
      // Browser mock mode or IPC unavailable — live events may still append below.
    }
  }, [scanId]);

  useEffect(() => {
    setContent("");
    offsetRef.current = 0;
    tailingRef.current = true;
    let unlisten: UnlistenFn | undefined;
    let pollTimer: ReturnType<typeof setInterval> | undefined;
    let cancelled = false;

    void appendTail();

    pollTimer = setInterval(() => {
      void appendTail();
    }, TAIL_POLL_MS);

    void listen<ScanProgressEvent>("scan-progress", (payload) => {
      if (payload.payload.scanId !== scanId) return;
      void appendTail();
    }).then((fn) => {
      if (cancelled) {
        void fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      tailingRef.current = false;
      if (pollTimer) clearInterval(pollTimer);
      void unlisten?.();
    };
  }, [appendTail, scanId]);

  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    body.scrollTop = body.scrollHeight;
  }, [content]);

  return (
    <div className="scan-console">
      <h4 className="scan-console__title">Attack Console</h4>
      <pre ref={bodyRef} className="scan-console__body" aria-live="polite">
        {content.length === 0 ? (
          <span className="scan-console__placeholder text-muted">Waiting for scan events…</span>
        ) : (
          content
        )}
      </pre>
    </div>
  );
}
