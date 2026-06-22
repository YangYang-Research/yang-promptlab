import { useCallback, useEffect, useState } from "react";

import { Button, Card, PageHeader, StatusBadge } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  getRuntimeHardware,
  getRuntimeHealth,
  getRuntimeLogs,
  getRuntimeStatus,
  refreshRuntimeHardware,
  restartRuntime,
  runRuntimeBenchmark,
  stopRuntime,
  type RuntimeBenchmarkResult,
  type RuntimeHardwareDto,
  type RuntimeHealthReport,
  type RuntimeLogEntry,
  type RuntimeStatusDto,
} from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";
import { formatBytes } from "@/shared/utils/format";

function lifecycleBadge(state: string): string {
  if (state === "running" || state === "busy") return "running";
  if (state === "installed" || state === "starting") return "completed";
  if (state === "failed" || state === "not_installed") return "failed";
  if (state === "stopped" || state === "stopping") return "cancelled";
  if (state === "downloading" || state === "installing" || state === "updating") return "running";
  return "pending";
}

function lifecycleLabel(state: string): string {
  return state.replace(/_/g, " ");
}

export function AIRuntimePage() {
  const { notify } = useToast();
  const [backendConnected, setBackendConnected] = useState(false);
  const [status, setStatus] = useState<RuntimeStatusDto | null>(null);
  const [hardware, setHardware] = useState<RuntimeHardwareDto | null>(null);
  const [health, setHealth] = useState<RuntimeHealthReport | null>(null);
  const [benchmark, setBenchmark] = useState<RuntimeBenchmarkResult | null>(null);
  const [logs, setLogs] = useState<RuntimeLogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showLogs, setShowLogs] = useState(false);

  const refresh = useCallback(async () => {
    const [runtime, hw, healthReport, logEntries] = await Promise.all([
      getRuntimeStatus(),
      getRuntimeHardware(),
      getRuntimeHealth(),
      getRuntimeLogs(50),
    ]);
    setStatus(runtime);
    setHardware(hw);
    setHealth(healthReport);
    setLogs(logEntries);
  }, []);

  useEffect(() => {
    void import("@/shared/ipc/client").then(({ healthCheck }) =>
      healthCheck()
        .then(() => setBackendConnected(true))
        .catch(() => setBackendConnected(false)),
    );
  }, []);

  useEffect(() => {
    if (!backendConnected) {
      setLoading(false);
      return;
    }
    void refresh()
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected, refresh]);

  useEffect(() => {
    if (!backendConnected) return;
    const timer = window.setInterval(() => {
      void refresh().catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [backendConnected, refresh]);

  async function runAction(label: string, fn: () => Promise<unknown>) {
    setError(null);
    setBusy(label);
    try {
      await fn();
      await refresh();
      notify(`${label} completed`, "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  const lifecycle = status?.lifecycleState ?? "not_installed";

  return (
    <div className="page">
      <PageHeader
        title="AI Runtime"
        description="Embedded llama.cpp inference runtime — installation, health, and lifecycle"
        actions={
          <Button
            variant="secondary"
            disabled={!backendConnected || loading || busy !== null}
            onClick={() => void refresh().catch((err) => setError(toAppError(err).message))}
          >
            Refresh
          </Button>
        }
      />

      {!backendConnected && (
        <p className="text-muted">Connect to the Tauri backend to manage the AI runtime.</p>
      )}

      {backendConnected && (
        <>
          <section className="runtime-section">
            <h2 className="runtime-section__title">Status</h2>
            <div className="models-summary-grid runtime-summary-grid">
              <Card className="models-summary-card">
                <span className="models-summary-card__label">Lifecycle</span>
                <div className="runtime-summary-card__value-row">
                  <strong className="models-summary-card__value models-summary-card__value--runtime">
                    {lifecycleLabel(lifecycle)}
                  </strong>
                  <StatusBadge status={lifecycleBadge(lifecycle)} />
                </div>
                <p className="text-muted text-sm">{status?.message ?? "Loading runtime status…"}</p>
              </Card>
              <Card className="models-summary-card">
                <span className="models-summary-card__label">Installed</span>
                <strong className="models-summary-card__value">
                  {status?.installed ? "Yes" : "No"}
                </strong>
                <p className="text-muted text-sm">
                  {status?.verified ? "SHA256 verified" : "Not verified"}
                </p>
              </Card>
              <Card className="models-summary-card">
                <span className="models-summary-card__label">Health</span>
                <strong className="models-summary-card__value">
                  {health?.endpointReachable ? "Reachable" : health?.processAlive ? "Process up" : "Idle"}
                </strong>
                <p className="text-muted text-sm">
                  {health?.latencyMs != null ? `${health.latencyMs} ms probe` : "—"}
                </p>
              </Card>
              <Card className="models-summary-card">
                <span className="models-summary-card__label">Backend</span>
                <strong className="models-summary-card__value">{status?.backend ?? "—"}</strong>
                <p className="text-muted text-sm mono">{status?.runtimeVersion ?? "—"}</p>
              </Card>
            </div>
          </section>

          <section className="runtime-section">
            <h2 className="runtime-section__title">Hardware</h2>
            <Card className="model-card model-card--wide">
              {hardware ? (
                <dl className="runtime-kv-grid">
                  <div><dt>OS</dt><dd>{hardware.os} / {hardware.arch}</dd></div>
                  <div><dt>CPU</dt><dd>{hardware.cpu} ({hardware.cpuCores} cores)</dd></div>
                  <div><dt>RAM</dt><dd>{formatBytes(hardware.ramBytes)}</dd></div>
                  <div><dt>GPU</dt><dd>{hardware.gpuName ?? "None detected"}</dd></div>
                  <div><dt>VRAM</dt><dd>{hardware.vramBytes ? formatBytes(hardware.vramBytes) : "—"}</dd></div>
                  <div><dt>Backends</dt><dd>
                    {[hardware.metal && "Metal", hardware.cuda && "CUDA", hardware.vulkan && "Vulkan", hardware.avx2 && "AVX2"]
                      .filter(Boolean)
                      .join(", ") || "CPU"}
                  </dd></div>
                  <div><dt>Detected</dt><dd>{hardware.detectedAt}</dd></div>
                </dl>
              ) : (
                <p className="text-muted">Hardware profile not available yet.</p>
              )}
              <div className="model-card__actions">
                <Button
                  variant="secondary"
                  disabled={loading || busy !== null}
                  onClick={() =>
                    void runAction("Hardware refresh", async () => {
                      setHardware(await refreshRuntimeHardware());
                    })
                  }
                >
                  {busy === "Hardware refresh" ? "Refreshing…" : "Refresh Hardware"}
                </Button>
              </div>
            </Card>
          </section>

          <section className="runtime-section">
            <h2 className="runtime-section__title">Runtime</h2>
            <Card className="model-card model-card--wide">
              <dl className="runtime-kv-grid">
                <div><dt>Install path</dt><dd className="mono">{status?.installPath ?? "—"}</dd></div>
                <div><dt>Platform</dt><dd>{status?.platform ?? "—"}</dd></div>
                <div><dt>API endpoint</dt><dd className="mono">{status?.baseUrl ?? "—"}</dd></div>
                <div><dt>Binary</dt><dd>{status?.binaryAvailable ? "Available" : "Missing"}</dd></div>
              </dl>
              <p className="text-muted text-sm">
                Runtime installs and starts automatically on app launch. Model activation is managed
                exclusively by the Models module.
              </p>
              <div className="model-card__actions">
                <Button
                  variant="secondary"
                  disabled={loading || busy !== null}
                  onClick={() => void runAction("Restart runtime", () => restartRuntime())}
                >
                  {busy === "Restart runtime" ? "Restarting…" : "Restart Runtime"}
                </Button>
                <Button
                  variant="ghost"
                  disabled={loading || busy !== null || lifecycle === "stopped"}
                  onClick={() => void runAction("Stop runtime", () => stopRuntime())}
                >
                  {busy === "Stop runtime" ? "Stopping…" : "Stop Runtime"}
                </Button>
              </div>
            </Card>
          </section>

          <section className="runtime-section">
            <h2 className="runtime-section__title">Benchmark</h2>
            <Card className="model-card model-card--wide">
              {benchmark ? (
                <dl className="runtime-kv-grid">
                  <div><dt>Latency</dt><dd>{benchmark.latencyMs} ms</dd></div>
                  <div><dt>Throughput</dt><dd>{benchmark.tokensPerSec.toFixed(1)} tok/s</dd></div>
                  <div><dt>Tokens</dt><dd>{benchmark.tokensPredicted}</dd></div>
                  <div><dt>Result</dt><dd>{benchmark.message}</dd></div>
                </dl>
              ) : (
                <p className="text-muted text-sm">
                  Run a benchmark after a model is loaded via the Models module.
                </p>
              )}
              <div className="model-card__actions">
                <Button
                  variant="secondary"
                  disabled={loading || busy !== null}
                  onClick={() =>
                    void runAction("Benchmark", async () => {
                      setBenchmark(await runRuntimeBenchmark());
                    })
                  }
                >
                  {busy === "Benchmark" ? "Running…" : "Run Benchmark"}
                </Button>
              </div>
            </Card>
          </section>

          <section className="runtime-section">
            <div className="runtime-section__header">
              <h2 className="runtime-section__title">Logs</h2>
              <Button variant="ghost" onClick={() => setShowLogs((v) => !v)}>
                {showLogs ? "Hide Logs" : "View Logs"}
              </Button>
            </div>
            {showLogs && (
              <Card className="model-card model-card--wide runtime-log-card">
                {logs.length === 0 ? (
                  <p className="text-muted">No runtime logs yet.</p>
                ) : (
                  <ul className="runtime-log-list">
                    {logs.map((entry, index) => (
                      <li key={`${entry.timestamp}-${index}`} className="runtime-log-list__item">
                        <span className="runtime-log-list__time">{entry.timestamp}</span>
                        <span className={`runtime-log-list__level runtime-log-list__level--${entry.level}`}>
                          {entry.level}
                        </span>
                        <span>{entry.message}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </Card>
            )}
          </section>
        </>
      )}

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
