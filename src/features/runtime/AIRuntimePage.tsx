import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";

import { Button, Card, PageHeader, StatusBadge } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import {
  getRuntimeHardware,
  getRuntimeHealth,
  getRuntimeLogs,
  installRuntime,
  refreshRuntimeHardware,
  repairRuntime,
  restartRuntime,
  runRuntimeBenchmark,
  startRuntime,
  stopRuntime,
  RUNTIME_INSTALL_PROGRESS_EVENT,
  type AiInferenceModelOptionDto,
  type AiInferenceRoute,
  type RuntimeBenchmarkResult,
  type RuntimeConfigurationDto,
  type RuntimeHardwareDto,
  type RuntimeInstallProgressEvent,
  type RuntimeLogEntry,
  type RuntimeStatusDto,
} from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";
import { formatBytes } from "@/shared/utils/format";

const INSTALL_STEPS = [
  { id: "hardware", label: "Detect hardware" },
  { id: "package", label: "Select package" },
  { id: "download", label: "Download runtime" },
  { id: "install", label: "Install binaries" },
  { id: "verify", label: "Verify install" },
  { id: "complete", label: "Ready to start" },
] as const;

function lifecycleBadge(state: string): string {
  if (state === "running" || state === "busy") return "running";
  if (state === "installed") return "completed";
  if (state === "starting" || state === "downloading" || state === "installing" || state === "updating") {
    return "running";
  }
  if (state === "failed" || state === "not_installed") return "failed";
  if (state === "stopped" || state === "stopping") return "cancelled";
  return "pending";
}

function stepIndex(stepId: string): number {
  const idx = INSTALL_STEPS.findIndex((s) => s.id === stepId);
  return idx >= 0 ? idx : 0;
}

function RuntimeInstallProgress({
  progress,
  installing,
  error,
}: {
  progress: RuntimeInstallProgressEvent | null;
  installing: boolean;
  error: string | null;
}) {
  const activeIdx = progress ? stepIndex(progress.step) : installing ? 0 : -1;
  const phase = progress?.phase ?? 0;

  return (
    <Card className="model-card model-card--wide runtime-setup-card">
      <div
        className="runtime-setup-card__bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={phase}
        aria-label="Runtime install progress"
      >
        <div className="runtime-setup-card__bar-fill" style={{ width: `${phase}%` }} />
      </div>
      <p className="runtime-setup-card__status">
        {installing
          ? progress?.message ?? "Installing runtime…"
          : error ?? "Waiting…"}
      </p>
      <ol className="runtime-setup-steps">
        {INSTALL_STEPS.map((step, index) => {
          let state: "pending" | "active" | "done" | "error" = "pending";
          if (error && index === activeIdx) state = "error";
          else if (index < activeIdx) state = "done";
          else if (index === activeIdx && installing) state = "active";
          else if (!installing && progress?.step === "complete") state = "done";

          return (
            <li key={step.id} className={`runtime-setup-steps__item runtime-setup-steps__item--${state}`}>
              <span className="runtime-setup-steps__marker" aria-hidden="true" />
              <span>{step.label}</span>
            </li>
          );
        })}
      </ol>
    </Card>
  );
}

type RuntimeModeOption = {
  route: AiInferenceRoute;
  badge: string;
  title: string;
  summary: string;
  highlights: string[];
  note: { label: string; body: string };
};

const RUNTIME_MODE_OPTIONS: RuntimeModeOption[] = [
  {
    route: "third_party",
    badge: "Cloud",
    title: "Third-party",
    summary: "Send AI requests to a cloud provider you configure.",
    highlights: [
      "OpenAI, Anthropic, AWS Bedrock, Azure, and more",
      "API keys stored securely on this device",
      "Traffic goes directly to the provider — not through AISec servers",
    ],
    note: {
      label: "Data privacy",
      body: "Prompts and responses are handled by your chosen provider. Check their retention and training policies before use.",
    },
  },
  {
    route: "local",
    badge: "On-device",
    title: "Local",
    summary: "Run GGUF models locally with the embedded llama.cpp runtime.",
    highlights: [
      "Inference stays on your machine",
      "No third-party sharing of prompts or outputs",
      "You control hardware setup, models, and startup",
    ],
    note: {
      label: "Minimum hardware",
      body: "8 GB RAM and ~6 GB free disk for the runtime plus a compact Q4 model. For larger models, use 16 GB RAM or Apple Silicon / CUDA.",
    },
  },
];

function RuntimeModePicker({
  disabled,
  onSelect,
}: {
  disabled: boolean;
  onSelect: (route: AiInferenceRoute) => void;
}) {
  return (
    <div className="runtime-mode-picker">
      <p className="runtime-mode-picker__lead">
        Pick how AISec runs AI features. You can change this later after configuration.
      </p>
      <div className="runtime-mode-picker__grid">
        {RUNTIME_MODE_OPTIONS.map((option) => (
          <button
            key={option.route}
            type="button"
            className={`runtime-mode-picker__card runtime-mode-picker__card--${option.route}`}
            disabled={disabled}
            onClick={() => onSelect(option.route)}
          >
            <Card className="runtime-mode-picker__card-inner" padding="md">
              <div className="runtime-mode-picker__card-header">
                <span className="runtime-mode-picker__badge">{option.badge}</span>
                <h3 className="runtime-mode-picker__card-title">{option.title}</h3>
                <p className="runtime-mode-picker__card-summary">{option.summary}</p>
              </div>

              <ul className="runtime-mode-picker__highlights">
                {option.highlights.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>

              <div className="runtime-mode-picker__note">
                <span className="runtime-mode-picker__note-label">{option.note.label}</span>
                <p className="runtime-mode-picker__note-body">{option.note.body}</p>
              </div>
            </Card>
          </button>
        ))}
      </div>
    </div>
  );
}

function ModeToggle({
  mode,
  disabled,
  onChange,
  compact = false,
}: {
  mode: RuntimeConfigurationDto["mode"];
  disabled: boolean;
  onChange: (route: AiInferenceRoute) => void;
  compact?: boolean;
}) {
  const thirdPartyActive = mode === "third_party";
  const localActive = mode === "local";

  return (
    <div
      className={`runtime-route-toggle${compact ? " runtime-route-toggle--header" : ""}`}
      role="group"
      aria-label="AI runtime mode"
    >
      <button
        type="button"
        className={`runtime-route-toggle__btn${thirdPartyActive ? " runtime-route-toggle__btn--active" : ""}`}
        disabled={disabled}
        aria-pressed={thirdPartyActive}
        onClick={() => onChange("third_party")}
      >
        Third-party
      </button>
      <button
        type="button"
        className={`runtime-route-toggle__btn${localActive ? " runtime-route-toggle__btn--active" : ""}`}
        disabled={disabled}
        aria-pressed={localActive}
        onClick={() => onChange("local")}
      >
        Local
      </button>
    </div>
  );
}

export function AIRuntimePage() {
  const { notify } = useToast();
  const navigate = useNavigate();
  const [backendConnected, setBackendConnected] = useState(false);
  const [hardware, setHardware] = useState<RuntimeHardwareDto | null>(null);
  const [benchmark, setBenchmark] = useState<RuntimeBenchmarkResult | null>(null);
  const [logs, setLogs] = useState<RuntimeLogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showLogs, setShowLogs] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installProgress, setInstallProgress] = useState<RuntimeInstallProgressEvent | null>(null);
  const [testingModelId, setTestingModelId] = useState<string | null>(null);

  const {
    configuration,
    settings,
    mode,
    loading: configLoading,
    busy: routeBusy,
    refresh: refreshConfiguration,
    setRoute,
  } = useAiInferenceRoute({ enabled: backendConnected });

  const status: RuntimeStatusDto | null = configuration?.runtimeStatus ?? null;
  const lifecycle = status?.lifecycleState ?? "not_installed";

  const refreshLocalData = useCallback(async () => {
    const [hw, logEntries] = await Promise.all([
      getRuntimeHardware(),
      getRuntimeLogs(50),
    ]);
    setHardware(hw);
    setLogs(logEntries);
  }, []);

  const refreshAll = useCallback(async () => {
    await Promise.all([refreshConfiguration(), refreshLocalData()]);
  }, [refreshConfiguration, refreshLocalData]);

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
    void refreshAll()
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected, refreshAll]);

  useEffect(() => {
    if (!backendConnected) return;
    let unlisten: (() => void) | undefined;
    void listen<RuntimeInstallProgressEvent>(RUNTIME_INSTALL_PROGRESS_EVENT, (event) => {
      setInstallProgress(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      void unlisten?.();
    };
  }, [backendConnected]);

  async function runAction(label: string, fn: () => Promise<unknown>) {
    setError(null);
    setBusy(label);
    try {
      await fn();
      await refreshAll();
      notify(`${label} completed`, "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  async function handleModeChange(route: AiInferenceRoute) {
    if (routeBusy || busy !== null || installing) return;
    try {
      await setRoute(route);
      await refreshLocalData();
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    }
  }

  async function handleSelectThirdPartyModel(modelId: string) {
    if (!backendConnected || loading || busy !== null || installing || routeBusy || testingModelId !== null) {
      return;
    }
    setError(null);
    setTestingModelId(modelId);
    try {
      const result = await setRoute("third_party", modelId);
      if (result?.settings.connectivityTestOk === false && result.settings.connectivityTestDetail) {
        notify(result.settings.connectivityTestDetail, "error");
      }
    } catch (err) {
      const message = toAppError(err).message;
      notify(message, "error");
    } finally {
      setTestingModelId(null);
    }
  }

  async function runInstall(installFn: () => Promise<RuntimeStatusDto>) {
    if (installing) return;
    setError(null);
    setInstalling(true);
    setInstallProgress({ step: "hardware", message: "Starting install…", phase: 5 });
    try {
      await installFn();
      setInstallProgress({ step: "complete", message: "Runtime installed", phase: 100 });
      notify("Runtime installed — press Start Runtime when ready", "success");
      await refreshAll();
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
      await refreshAll().catch(() => undefined);
    } finally {
      setInstalling(false);
    }
  }

  const disabled = !backendConnected || loading || busy !== null || installing || routeBusy;
  const localModels = settings?.localModels ?? [];
  const thirdPartyModels = settings?.thirdPartyModels ?? [];
  const showModePicker = backendConnected && mode === "not_configured" && !configLoading;
  const modeConfigured = mode === "third_party" || mode === "local";

  return (
    <div className={`page${showModePicker ? " page--runtime-setup" : ""}`}>
      <PageHeader
        title="AI Runtime"
        description={
          showModePicker
            ? "Set up how AISec interacts with AI models."
            : "Configure third-party cloud providers or manage the embedded local llama.cpp runtime"
        }
        actions={
          backendConnected ? (
            <div className="page-header__actions-row">
              {modeConfigured && !configLoading && (
                <ModeToggle
                  mode={mode}
                  disabled={disabled}
                  onChange={handleModeChange}
                  compact
                />
              )}
              <Button
                variant="secondary"
                disabled={disabled || configLoading}
                onClick={() => void refreshAll().catch((err) => setError(toAppError(err).message))}
              >
                Refresh
              </Button>
            </div>
          ) : undefined
        }
      />

      {!backendConnected && (
        <p className="text-muted">Connect to the Tauri backend to manage the AI runtime.</p>
      )}

      {configLoading && backendConnected && (
        <p className="text-muted text-sm">Loading runtime configuration…</p>
      )}

      {showModePicker && (
        <RuntimeModePicker disabled={disabled} onSelect={handleModeChange} />
      )}

      {backendConnected && modeConfigured && (
        <>
          {mode === "third_party" && (
            <>
              <section className="runtime-section">
                <div className="models-summary-grid runtime-summary-grid">
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Status</span>
                    <strong className="models-summary-card__value models-summary-card__value--runtime">
                      {configuration?.statusLabel ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Provider</span>
                    <strong className="models-summary-card__value">
                      {configuration?.provider ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Model</span>
                    <strong className="models-summary-card__value models-summary-card__value--runtime">
                      {configuration?.modelName ?? settings?.selectedModelName ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Connectivity</span>
                    <strong className="models-summary-card__value">
                      {testingModelId
                        ? "Testing…"
                        : configuration?.connectivity ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Last Health Check</span>
                    <strong className="models-summary-card__value models-summary-card__value--runtime">
                      {testingModelId
                        ? "Running connection test…"
                        : configuration?.lastHealthCheck ?? "Not checked"}
                    </strong>
                  </Card>
                </div>
              </section>

              <section className="runtime-section">
                <h2 className="runtime-section__title">Choose Model</h2>
                <Card className="model-card model-card--wide">
                  {thirdPartyModels.length > 0 && (
                    <ul className="runtime-route-models" aria-label="Third-party models">
                      {thirdPartyModels.map((model) => (
                        <ThirdPartyModelRow
                          key={model.id}
                          model={model}
                          selected={model.id === settings?.selectedModelId}
                          disabled={disabled}
                          testing={testingModelId === model.id}
                          onSelect={() => void handleSelectThirdPartyModel(model.id)}
                        />
                      ))}
                    </ul>
                  )}
                  <div className="model-card__actions">
                    <Button
                      onClick={() =>
                        navigate("/models", {
                          state: { openAddModel: true, openAddModelTab: "third-party" },
                        })
                      }
                    >
                      Add Model
                    </Button>
                  </div>
                </Card>
              </section>
            </>
          )}

          {mode === "local" && (
            <>
              <section className="runtime-section">
                <h2 className="runtime-section__title">Status</h2>
                <div className="models-summary-grid runtime-summary-grid">
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Current Status</span>
                    <div className="runtime-summary-card__value-row">
                      <strong className="models-summary-card__value models-summary-card__value--runtime">
                        {configuration?.statusLabel ?? lifecycle.replace(/_/g, " ")}
                      </strong>
                      <StatusBadge status={lifecycleBadge(lifecycle)} />
                    </div>
                    <p className="text-muted text-sm">{status?.message ?? "—"}</p>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Current Runtime</span>
                    <strong className="models-summary-card__value">
                      {configuration?.runtimeName ?? status?.backend ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Installed Version</span>
                    <strong className="models-summary-card__value">
                      {configuration?.runtimeVersion ?? status?.runtimeVersion ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Health</span>
                    <strong className="models-summary-card__value">
                      {configuration?.connectivity ?? "Not checked"}
                    </strong>
                    <p className="text-muted text-sm">
                      {configuration?.lastHealthCheck ?? "Run Start Runtime to probe health"}
                    </p>
                  </Card>
                </div>
              </section>

              <section className="runtime-section">
                <h2 className="runtime-section__title">Hardware</h2>
                <Card className="model-card model-card--wide">
                  {hardware ? (
                    <dl className="runtime-kv-grid">
                      <div><dt>CPU</dt><dd>{hardware.cpu} ({hardware.cpuCores} cores)</dd></div>
                      <div><dt>CUDA</dt><dd>{hardware.cuda ? "Yes" : "No"}</dd></div>
                      <div><dt>Metal</dt><dd>{hardware.metal ? "Yes" : "No"}</dd></div>
                      <div><dt>ROCm</dt><dd>No</dd></div>
                      <div><dt>RAM</dt><dd>{formatBytes(hardware.ramBytes)}</dd></div>
                      <div><dt>GPU</dt><dd>{hardware.gpuName ?? "None detected"}</dd></div>
                    </dl>
                  ) : (
                    <p className="text-muted text-sm">
                      Hardware not detected yet. Press Detect Hardware to profile this machine.
                    </p>
                  )}
                  <div className="model-card__actions">
                    <Button
                      variant="secondary"
                      disabled={disabled}
                      onClick={() =>
                        void runAction("Detect hardware", async () => {
                          setHardware(await refreshRuntimeHardware());
                        })
                      }
                    >
                      {busy === "Detect hardware" ? "Detecting…" : "Detect Hardware"}
                    </Button>
                  </div>
                </Card>
              </section>

              <section className="runtime-section">
                <h2 className="runtime-section__title">Runtime</h2>
                <Card className="model-card model-card--wide">
                  <dl className="runtime-kv-grid">
                    <div>
                      <dt>Recommended Runtime</dt>
                      <dd>{status?.recommendedRuntime ?? "Detect hardware first"}</dd>
                    </div>
                    <div><dt>Install path</dt><dd className="mono">{status?.installPath ?? "—"}</dd></div>
                    <div><dt>API endpoint</dt><dd className="mono">{status?.baseUrl ?? "—"}</dd></div>
                    <div><dt>Binary</dt><dd>{status?.binaryAvailable ? "Available" : "Missing"}</dd></div>
                  </dl>

                  {installing && (
                    <RuntimeInstallProgress
                      progress={installProgress}
                      installing={installing}
                      error={error}
                    />
                  )}

                  <div className="model-card__actions">
                    <Button
                      disabled={disabled}
                      onClick={() => void runInstall(() => installRuntime())}
                    >
                      {installing ? "Installing…" : "Install Runtime"}
                    </Button>
                    <Button
                      variant="secondary"
                      disabled={disabled}
                      onClick={() => void runInstall(() => repairRuntime())}
                    >
                      Repair Runtime
                    </Button>
                    <Button
                      variant="secondary"
                      disabled={disabled || !status?.installed}
                      onClick={() => void runAction("Start runtime", () => startRuntime())}
                    >
                      {busy === "Start runtime" ? "Starting…" : "Start Runtime"}
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={disabled || lifecycle === "stopped" || lifecycle === "not_installed"}
                      onClick={() => void runAction("Stop runtime", () => stopRuntime())}
                    >
                      {busy === "Stop runtime" ? "Stopping…" : "Stop Runtime"}
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={disabled || !status?.installed}
                      onClick={() => void runAction("Restart runtime", () => restartRuntime())}
                    >
                      {busy === "Restart runtime" ? "Restarting…" : "Restart Runtime"}
                    </Button>
                    <Button variant="ghost" onClick={() => setShowLogs((v) => !v)}>
                      {showLogs ? "Hide Logs" : "Logs"}
                    </Button>
                  </div>

                  {showLogs && (
                    <div className="runtime-log-card runtime-log-card--inline">
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
                    </div>
                  )}
                </Card>
              </section>

              <section className="runtime-section">
                <h2 className="runtime-section__title">Installed Models</h2>
                <Card className="model-card model-card--wide">
                  <dl className="runtime-kv-grid">
                    <div>
                      <dt>Current Model</dt>
                      <dd>{configuration?.modelName ?? status?.loadedModelPath ?? "—"}</dd>
                    </div>
                  </dl>
                  {localModels.length === 0 ? (
                    <p className="text-muted text-sm">No local models installed yet.</p>
                  ) : (
                    <ul className="runtime-route-models" aria-label="Installed local models">
                      {localModels.map((model) => (
                        <li
                          key={model.id}
                          className={`runtime-route-models__item${
                            model.id === settings?.selectedModelId
                              ? " runtime-route-models__item--selected"
                              : ""
                          }`}
                        >
                          <div>
                            <div className="runtime-route-models__name">{model.name}</div>
                            <div className="runtime-route-models__meta">
                              {model.provider}
                              {model.configured ? " · ready" : " · needs setup"}
                            </div>
                          </div>
                        </li>
                      ))}
                    </ul>
                  )}
                  <div className="model-card__actions">
                    <Button
                      variant="secondary"
                      onClick={() =>
                        navigate("/models", {
                          state: { openAddModel: true, openAddModelTab: "public" },
                        })
                      }
                    >
                      Manage Models
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={disabled}
                      onClick={() =>
                        void runAction("Health check", async () => {
                          await getRuntimeHealth();
                        })
                      }
                    >
                      {busy === "Health check" ? "Checking…" : "Health Check"}
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={disabled}
                      onClick={() =>
                        void runAction("Benchmark", async () => {
                          setBenchmark(await runRuntimeBenchmark());
                        })
                      }
                    >
                      {busy === "Benchmark" ? "Running…" : "Benchmark"}
                    </Button>
                  </div>
                  {benchmark && (
                    <dl className="runtime-kv-grid">
                      <div><dt>Benchmark latency</dt><dd>{benchmark.latencyMs} ms</dd></div>
                      <div><dt>Throughput</dt><dd>{benchmark.tokensPerSec.toFixed(1)} tok/s</dd></div>
                    </dl>
                  )}
                </Card>
              </section>
            </>
          )}

          {error && <p className="text-danger">{error}</p>}
        </>
      )}
    </div>
  );
}

function ThirdPartyModelRow({
  model,
  selected,
  disabled,
  testing = false,
  onSelect,
}: {
  model: AiInferenceModelOptionDto;
  selected: boolean;
  disabled: boolean;
  testing?: boolean;
  onSelect: () => void;
}) {
  return (
    <li
      className={`runtime-route-models__item${selected ? " runtime-route-models__item--selected" : ""}`}
    >
      <div>
        <div className="runtime-route-models__name">{model.name}</div>
        <div className="runtime-route-models__meta">
          {model.provider}
          {model.configured ? " · ready" : " · needs setup"}
        </div>
      </div>
      {!selected && (
        <button
          type="button"
          className="runtime-route-models__pick"
          disabled={disabled || testing}
          onClick={onSelect}
        >
          {testing ? "Testing…" : "Use"}
        </button>
      )}
    </li>
  );
}
