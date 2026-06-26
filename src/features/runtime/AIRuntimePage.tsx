import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";

import { Button, Card, ConnectivityStatus, Modal, PageHeader, RefreshButton } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import { useRuntimeModelLoading } from "@/shared/hooks/useRuntimeModelLoading";
import {
  getRuntimeHardware,
  getRuntimeHealth,
  getRuntimeLogs,
  loadRuntimeModel,
  reinitializeRuntimeEngine,
  resetRuntimeConfig,
  restartRuntime,
  runRuntimeBenchmark,
  startRuntime,
  stopRuntime,
  unloadRuntimeModel,
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

const ENGINE_INIT_STEPS = [
  { id: "hardware", label: "Detect hardware" },
  { id: "runtime", label: "Initialize engine" },
  { id: "complete", label: "Ready" },
] as const;

function stepIndex(stepId: string): number {
  const idx = ENGINE_INIT_STEPS.findIndex((s) => s.id === stepId);
  return idx >= 0 ? idx : 0;
}

function localInferenceLabel(
  status: RuntimeStatusDto | null,
  connectivity: string | null | undefined,
  statusLabel: string | undefined,
  modelLoadInProgress: boolean,
): string {
  if (modelLoadInProgress) return "Loading model";
  if (connectivity) return connectivity;
  if (statusLabel) return statusLabel;
  if (!status) return "Not checked";
  if (!status.binaryAvailable) {
    return status.lifecycleState === "not_installed" ? "Not initialized" : "Unavailable";
  }
  return status.lifecycleState.replace(/_/g, " ");
}

function RuntimeEngineProgress({
  progress,
  initializing,
  error,
}: {
  progress: RuntimeInstallProgressEvent | null;
  initializing: boolean;
  error: string | null;
}) {
  const activeIdx = progress ? stepIndex(progress.step) : initializing ? 0 : -1;
  const phase = progress?.phase ?? 0;

  return (
    <Card className="model-card model-card--wide runtime-setup-card">
      <div
        className="runtime-setup-card__bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={phase}
        aria-label="Engine initialization progress"
      >
        <div className="runtime-setup-card__bar-fill" style={{ width: `${phase}%` }} />
      </div>
      <p className="runtime-setup-card__status">
        {initializing
          ? progress?.message ?? "Initializing engine…"
          : error ?? "Waiting…"}
      </p>
      <ol className="runtime-setup-steps">
        {ENGINE_INIT_STEPS.map((step, index) => {
          let state: "pending" | "active" | "done" | "error" = "pending";
          if (error && index === activeIdx) state = "error";
          else if (index < activeIdx) state = "done";
          else if (index === activeIdx && initializing) state = "active";
          else if (!initializing && progress?.step === "complete") state = "done";

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

type RuntimeModeNote = {
  label: "Data privacy" | "Hardware";
  body: string;
};

type RuntimeModeOption = {
  route: AiInferenceRoute;
  title: string;
  badge: string;
  summary: string;
  highlights: string[];
  notes: RuntimeModeNote[];
};

const RUNTIME_MODE_OPTIONS: RuntimeModeOption[] = [
  {
    route: "third_party",
    title: "Third-party Providers",
    badge: "Remote",
    summary: "Route AI requests to a cloud provider you register in Models.",
    highlights: [
      "OpenAI, Anthropic, AWS Bedrock, Google, Azure, and custom endpoints",
      "API keys stored on this device (OS keychain when available)",
      "Traffic goes directly to the provider — not through AISec servers",
    ],
    notes: [
      {
        label: "Data privacy",
        body: "Prompts and responses are handled by your chosen provider. Review their retention, logging, and training policies before sending sensitive content.",
      },
      {
        label: "Hardware",
        body: "No local GPU required. A stable internet connection is enough; response time depends on provider region and model size.",
      },
    ],
  },
  {
    route: "local",
    title: "Local Runtime",
    badge: "On-device",
    summary: "Run GGUF models on this machine with the bundled llama.cpp runtime.",
    highlights: [
      "Inference runs entirely on your hardware",
      "Prompts and outputs stay on this device",
      "You control model selection and engine startup",
    ],
    notes: [
      {
        label: "Data privacy",
        body: "All inference stays on-device. AISec does not send prompts or model outputs to third-party services in this mode.",
      },
      {
        label: "Hardware",
        body: "8 GB RAM and ~6 GB free disk for the runtime plus a compact Q4 model. Larger models need 16 GB+ RAM or Apple Silicon / CUDA acceleration.",
      },
    ],
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
      <h2 className="runtime-mode-picker__title">Choose AI Runtime Mode</h2>
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
                <div className="runtime-mode-picker__card-header-top">
                  <span className="runtime-mode-picker__badge">{option.badge}</span>
                </div>
                <h3 className="runtime-mode-picker__card-title">{option.title}</h3>
                <p className="runtime-mode-picker__card-summary">{option.summary}</p>
              </div>

              <ul className="runtime-mode-picker__highlights">
                {option.highlights.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>

              <div className="runtime-mode-picker__notes">
                {option.notes.map((note) => (
                  <div key={note.label} className="runtime-mode-picker__note">
                    <span className="runtime-mode-picker__note-label">{note.label}</span>
                    <p className="runtime-mode-picker__note-body">{note.body}</p>
                  </div>
                ))}
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
  disableThirdParty = false,
  disableLocal = false,
  onChange,
  compact = false,
}: {
  mode: RuntimeConfigurationDto["mode"];
  disabled: boolean;
  disableThirdParty?: boolean;
  disableLocal?: boolean;
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
        disabled={disabled || disableThirdParty}
        aria-pressed={thirdPartyActive}
        onClick={() => onChange("third_party")}
      >
        Third-party
      </button>
      <button
        type="button"
        className={`runtime-route-toggle__btn${localActive ? " runtime-route-toggle__btn--active" : ""}`}
        disabled={disabled || disableLocal}
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
  const [engineInitializing, setEngineInitializing] = useState(false);
  const [engineInitProgress, setEngineInitProgress] = useState<RuntimeInstallProgressEvent | null>(null);
  const [testingModelId, setTestingModelId] = useState<string | null>(null);
  const [modelRuntimeAction, setModelRuntimeAction] = useState<{
    id: string;
    action: "load" | "unload";
  } | null>(null);
  const [loadModelConfirm, setLoadModelConfirm] = useState<{
    targetId: string;
    targetName: string;
    loadedName: string;
  } | null>(null);
  const [unloadModelConfirm, setUnloadModelConfirm] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const {
    configuration,
    settings,
    mode,
    loading: configLoading,
    busy: routeBusy,
    error: configError,
    refresh: refreshConfiguration,
    setRoute,
  } = useAiInferenceRoute({ enabled: backendConnected });

  const { modelLoading: runtimeModelLoading, loadingModelId, refresh: refreshModelLoading } =
    useRuntimeModelLoading(backendConnected);

  const status: RuntimeStatusDto | null = configuration?.runtimeStatus ?? null;
  const lifecycle = status?.lifecycleState ?? "not_installed";

  const refreshHardware = useCallback(async () => {
    setHardware(await getRuntimeHardware());
  }, []);

  const refreshLocalData = useCallback(async () => {
    const [logEntries] = await Promise.all([
      getRuntimeLogs(50).catch(() => [] as RuntimeLogEntry[]),
      refreshHardware(),
    ]);
    setLogs(logEntries);
  }, [refreshHardware]);

  const refreshAll = useCallback(async () => {
    await Promise.all([refreshConfiguration(), refreshLocalData()]);
  }, [refreshConfiguration, refreshLocalData]);

  const handleRefresh = useCallback(async () => {
    if (!backendConnected || refreshing) return;
    setError(null);
    setRefreshing(true);
    try {
      await refreshAll();
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setRefreshing(false);
    }
  }, [backendConnected, refreshing, refreshAll, notify]);

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
      setEngineInitProgress(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      void unlisten?.();
    };
  }, [backendConnected]);

  useEffect(() => {
    if (!backendConnected || (modelRuntimeAction === null && !runtimeModelLoading)) return;
    const timer = window.setInterval(() => {
      void Promise.all([refreshConfiguration(), refreshModelLoading()]).catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(timer);
  }, [
    backendConnected,
    modelRuntimeAction,
    runtimeModelLoading,
    refreshConfiguration,
    refreshModelLoading,
  ]);

  useEffect(() => {
    if (!backendConnected || runtimeModelLoading) return;
    void refreshConfiguration().catch(() => undefined);
  }, [backendConnected, runtimeModelLoading, refreshConfiguration]);

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
    if (routeBusy || busy !== null || engineInitializing) return;
    if (modelLoadInProgress) return;
    if (route === "third_party" && (runtimeModelLoading || modelRuntimeAction?.action === "load")) {
      return;
    }
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
    if (!backendConnected || loading || busy !== null || engineInitializing || routeBusy || testingModelId !== null) {
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

  function isLocalModelLoaded(modelId: string): boolean {
    return Boolean(status?.modelLoaded && settings?.selectedModelId === modelId);
  }

  async function runLoadLocalModel(modelId: string) {
    if (
      !backendConnected ||
      loading ||
      busy !== null ||
      engineInitializing ||
      routeBusy ||
      modelRuntimeAction !== null ||
      !status?.binaryAvailable ||
      !runtimeStarted
    ) {
      return;
    }
    setError(null);
    setModelRuntimeAction({ id: modelId, action: "load" });
    try {
      await loadRuntimeModel(modelId);
      await refreshAll();
      notify("Model loaded into runtime", "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setModelRuntimeAction(null);
    }
  }

  function handleLoadLocalModel(modelId: string) {
    if (
      !backendConnected ||
      loading ||
      busy !== null ||
      engineInitializing ||
      routeBusy ||
      modelRuntimeAction !== null ||
      modelLoadInProgress ||
      !status?.binaryAvailable ||
      !runtimeStarted
    ) {
      return;
    }
    if (isLocalModelLoaded(modelId)) {
      return;
    }
    const models = settings?.localModels ?? [];
    const selectedId = settings?.selectedModelId;
    if (status?.modelLoaded && selectedId && selectedId !== modelId) {
      const loaded = models.find((m) => m.id === selectedId);
      const target = models.find((m) => m.id === modelId);
      setLoadModelConfirm({
        targetId: modelId,
        targetName: target?.name ?? modelId,
        loadedName: loaded?.name ?? configuration?.modelName ?? "the loaded model",
      });
      return;
    }
    void runLoadLocalModel(modelId);
  }

  async function runUnloadLocalModel(modelId: string) {
    if (
      !backendConnected ||
      loading ||
      busy !== null ||
      engineInitializing ||
      routeBusy ||
      modelRuntimeAction !== null ||
      !isLocalModelLoaded(modelId)
    ) {
      return;
    }
    setError(null);
    setModelRuntimeAction({ id: modelId, action: "unload" });
    try {
      await unloadRuntimeModel();
      await refreshAll();
      notify("Model unloaded from runtime", "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setModelRuntimeAction(null);
    }
  }

  function handleUnloadLocalModel(modelId: string) {
    if (
      !backendConnected ||
      loading ||
      busy !== null ||
      engineInitializing ||
      routeBusy ||
      modelRuntimeAction !== null ||
      modelLoadInProgress ||
      !isLocalModelLoaded(modelId)
    ) {
      return;
    }
    const models = settings?.localModels ?? [];
    const model = models.find((m) => m.id === modelId);
    setUnloadModelConfirm({
      modelId,
      modelName: model?.name ?? configuration?.modelName ?? "this model",
    });
  }

  function modelRowLoadingAction(modelId: string): "load" | "unload" | null {
    if (modelRuntimeAction?.id === modelId) {
      return modelRuntimeAction.action;
    }
    if (runtimeModelLoading && (loadingModelId === null || loadingModelId === modelId)) {
      return "load";
    }
    return null;
  }

  async function runEngineReinitialize() {
    if (engineInitializing) return;
    setError(null);
    setEngineInitializing(true);
    setEngineInitProgress({ step: "hardware", message: "Initializing engine…", phase: 5 });
    try {
      await reinitializeRuntimeEngine();
      setEngineInitProgress({ step: "complete", message: "Engine ready", phase: 100 });
      notify("Inference engine ready — press Start Runtime when ready", "success");
      await refreshAll();
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
      await refreshAll().catch(() => undefined);
    } finally {
      setEngineInitializing(false);
    }
  }

  const disabled = !backendConnected || loading || busy !== null || engineInitializing || routeBusy;
  const modelLoadInProgress =
    runtimeModelLoading || modelRuntimeAction?.action === "load";
  const localRuntimeStatusLabel = modelLoadInProgress
    ? "Loading model"
    : (configuration?.statusLabel ?? lifecycle.replace(/_/g, " "));
  const inferenceLabel = localInferenceLabel(
    status,
    configuration?.connectivity,
    configuration?.statusLabel,
    modelLoadInProgress,
  );
  const runtimeStarted =
    lifecycle === "running" || lifecycle === "busy" || lifecycle === "starting";
  const localModelActionsDisabled =
    disabled ||
    modelLoadInProgress ||
    modelRuntimeAction !== null ||
    !status?.binaryAvailable ||
    !runtimeStarted;
  const runtimeConfigured = lifecycle !== "not_installed";
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
              <RefreshButton
                loading={refreshing || configLoading}
                error={error}
                disabled={!backendConnected}
                onClick={() => void handleRefresh()}
              />
              {modeConfigured && !configLoading && (
                <ModeToggle
                  mode={mode}
                  disabled={disabled}
                  disableThirdParty={modelLoadInProgress}
                  disableLocal={modelLoadInProgress}
                  onChange={handleModeChange}
                  compact
                />
              )}
            </div>
          ) : undefined
        }
      />

      {!backendConnected && (
        <p className="text-muted">Connect to the Tauri backend to manage the AI runtime.</p>
      )}

      {configLoading && !configuration && backendConnected && (
        <p className="text-muted text-sm">Loading runtime configuration…</p>
      )}

      {configError && !configuration && backendConnected && !configLoading && (
        <p className="text-danger text-sm">{configError}</p>
      )}

      {showModePicker && (
        <RuntimeModePicker
          disabled={disabled || modelLoadInProgress}
          onSelect={handleModeChange}
        />
      )}

      {backendConnected && (modeConfigured || configuration) && (
        <>
          {mode === "third_party" && (
            <>
              <section className="runtime-section">
                <h2 className="runtime-section__title">Status</h2>
                <div className="models-summary-grid runtime-summary-grid">
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Runtime Status</span>
                    <strong className="models-summary-card__value models-summary-card__value--runtime">
                      {configuration?.statusLabel ? (
                        <ConnectivityStatus label={configuration.statusLabel} />
                      ) : (
                        "—"
                      )}
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
                    <strong className="models-summary-card__value models-summary-card__value--model-name">
                      {configuration?.modelName ?? settings?.selectedModelName ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Connectivity</span>
                    <strong className="models-summary-card__value models-summary-card__value--wrap">
                      {testingModelId ? (
                        "Testing…"
                      ) : configuration?.connectivity ? (
                        <ConnectivityStatus label={configuration.connectivity} />
                      ) : (
                        "—"
                      )}
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
                <h2 className="runtime-section__title">Registered Models</h2>
                <Card className="model-card model-card--wide">
                  {thirdPartyModels.length > 0 ? (
                    <>
                      <ul className="runtime-route-models" aria-label="Registered third-party models">
                        {thirdPartyModels.map((model) => (
                          <ThirdPartyModelRow
                            key={model.id}
                            model={model}
                            selected={model.id === settings?.selectedModelId}
                            disabled={disabled}
                            testing={testingModelId === model.id}
                            onSelect={() => void handleSelectThirdPartyModel(model.id)}
                            onEdit={() =>
                              navigate("/models", { state: { editModelId: model.id } })
                            }
                          />
                        ))}
                      </ul>
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
                    </>
                  ) : (
                    <div className="runtime-choose-model__empty">
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
                  )}
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
                    <span className="models-summary-card__label">Runtime Status</span>
                    <strong className="models-summary-card__value models-summary-card__value--runtime">
                      {localRuntimeStatusLabel}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Current Runtime</span>
                    <strong className="models-summary-card__value">
                      {configuration?.runtimeName ?? "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Loaded Model</span>
                    <strong className="models-summary-card__value models-summary-card__value--model-name">
                      {status?.modelLoaded
                        ? (configuration?.modelName ?? status?.loadedModelPath ?? "—")
                        : "—"}
                    </strong>
                  </Card>
                  <Card className="models-summary-card">
                    <span className="models-summary-card__label">Health</span>
                    <strong className="models-summary-card__value">
                      {configuration?.connectivity ?? "Not checked"}
                    </strong>
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
                      Hardware profile not available yet. Reinitialize the engine to detect this machine.
                    </p>
                  )}
                </Card>
              </section>

              <section className="runtime-section">
                <h2 className="runtime-section__title">Inference Engine</h2>
                <Card className="model-card model-card--wide">
                  <p className="text-muted text-sm runtime-engine-intro">
                    Embedded libllama runs in-process — no separate runtime binary to install.
                  </p>
                  <dl className="runtime-kv-grid">
                    <div>
                      <dt>Engine</dt>
                      <dd>{status?.recommendedRuntime ?? "Reinitialize engine to profile hardware"}</dd>
                    </div>
                    <div>
                      <dt>Version</dt>
                      <dd>{configuration?.runtimeVersion ?? status?.runtimeVersion ?? "—"}</dd>
                    </div>
                    <div>
                      <dt>Inference</dt>
                      <dd>
                        <ConnectivityStatus label={inferenceLabel} />
                      </dd>
                    </div>
                    <div>
                      <dt>Status</dt>
                      <dd>{status?.message ?? "—"}</dd>
                    </div>
                  </dl>

                  {engineInitializing && (
                    <RuntimeEngineProgress
                      progress={engineInitProgress}
                      initializing={engineInitializing}
                      error={error}
                    />
                  )}

                  <div className="model-card__actions">
                    <Button
                      disabled={disabled}
                      onClick={() => void runEngineReinitialize()}
                    >
                      {engineInitializing ? "Initializing…" : "Reinitialize Engine"}
                    </Button>
                    {!runtimeStarted && (
                      <Button
                        variant="secondary"
                        disabled={disabled || !runtimeConfigured || !status?.binaryAvailable}
                        onClick={() => void runAction("Start runtime", () => startRuntime())}
                      >
                        {busy === "Start runtime" ? "Starting…" : "Start Runtime"}
                      </Button>
                    )}
                    {runtimeStarted && (
                      <Button
                        variant="ghost"
                        disabled={disabled}
                        onClick={() => void runAction("Stop runtime", () => stopRuntime())}
                      >
                        {busy === "Stop runtime" ? "Stopping…" : "Stop Runtime"}
                      </Button>
                    )}
                    {runtimeStarted && (
                      <Button
                        variant="ghost"
                        disabled={disabled}
                        onClick={() => void runAction("Restart runtime", () => restartRuntime())}
                      >
                        {busy === "Restart runtime" ? "Restarting…" : "Restart Runtime"}
                      </Button>
                    )}
                    <Button
                      variant="danger"
                      disabled={disabled || !runtimeConfigured}
                      onClick={() =>
                        void runAction("Reset runtime config", async () => {
                          await resetRuntimeConfig();
                        })
                      }
                    >
                      {busy === "Reset runtime config" ? "Resetting…" : "Reset Runtime Config"}
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
                  {localModels.length === 0 ? (
                    <p className="text-muted text-sm">No local models installed yet.</p>
                  ) : (
                    <ul className="runtime-route-models" aria-label="Installed local models">
                      {localModels.map((model) => (
                        <LocalModelRow
                          key={model.id}
                          model={model}
                          loaded={isLocalModelLoaded(model.id)}
                          disabled={localModelActionsDisabled}
                          loading={modelRowLoadingAction(model.id)}
                          onLoad={() => void handleLoadLocalModel(model.id)}
                          onUnload={() => void handleUnloadLocalModel(model.id)}
                        />
                      ))}
                    </ul>
                  )}
                  <div className="model-card__actions">
                    <Button
                      onClick={() =>
                        navigate("/models", {
                          state: { openAddModel: true, openAddModelTab: "public" },
                        })
                      }
                    >
                      Add Model
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

      <Modal
        open={loadModelConfirm !== null}
        title="Switch model in AI Runtime?"
        onClose={() => setLoadModelConfirm(null)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setLoadModelConfirm(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              disabled={modelRuntimeAction !== null}
              onClick={() => {
                const targetId = loadModelConfirm?.targetId;
                setLoadModelConfirm(null);
                if (targetId) void runLoadLocalModel(targetId);
              }}
            >
              Continue
            </Button>
          </>
        }
      >
        {loadModelConfirm ? (
          <p>
            <strong>{loadModelConfirm.loadedName}</strong> is active in AI Runtime. To load{" "}
            <strong>{loadModelConfirm.targetName}</strong>, the runtime will unload the current
            model and load this one first. Continue?
          </p>
        ) : null}
      </Modal>

      <Modal
        open={unloadModelConfirm !== null}
        title="Unload model from AI Runtime?"
        onClose={() => setUnloadModelConfirm(null)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setUnloadModelConfirm(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              disabled={modelRuntimeAction !== null}
              onClick={() => {
                const modelId = unloadModelConfirm?.modelId;
                setUnloadModelConfirm(null);
                if (modelId) void runUnloadLocalModel(modelId);
              }}
            >
              Unload
            </Button>
          </>
        }
      >
        {unloadModelConfirm ? (
          <p>
            <strong>{unloadModelConfirm.modelName}</strong> is active in AI Runtime. Unloading it
            will stop the inference API until you load a model again. Continue?
          </p>
        ) : null}
      </Modal>
    </div>
  );
}

function thirdPartyModelNeedsEdit(model: AiInferenceModelOptionDto): boolean {
  return model.statusLabel === "Connection Failed" || model.statusLabel === "Not Verified";
}

function LocalModelRow({
  model,
  loaded,
  disabled,
  loading,
  onLoad,
  onUnload,
}: {
  model: AiInferenceModelOptionDto;
  loaded: boolean;
  disabled: boolean;
  loading: "load" | "unload" | null;
  onLoad: () => void;
  onUnload: () => void;
}) {
  return (
    <li
      className={`runtime-route-models__item${loaded ? " runtime-route-models__item--selected" : ""}`}
    >
      <div className="runtime-route-models__info">
        <div className="runtime-route-models__name">{model.name}</div>
        <div className="runtime-route-models__meta">
          {model.provider}
          {` · ${model.statusLabel}`}
          {loaded ? " · Loaded" : ""}
        </div>
      </div>
      {!loaded && (
        <button
          type="button"
          className="runtime-route-models__pick"
          disabled={disabled || loading !== null || !model.configured}
          onClick={onLoad}
        >
          {loading === "load" ? "Loading…" : "Load"}
        </button>
      )}
      {loaded && (
        <button
          type="button"
          className="runtime-route-models__pick"
          disabled={disabled || loading !== null}
          onClick={onUnload}
        >
          {loading === "unload" ? "Unloading…" : "Unload"}
        </button>
      )}
    </li>
  );
}

function ThirdPartyModelRow({
  model,
  selected,
  disabled,
  testing = false,
  onSelect,
  onEdit,
}: {
  model: AiInferenceModelOptionDto;
  selected: boolean;
  disabled: boolean;
  testing?: boolean;
  onSelect: () => void;
  onEdit: () => void;
}) {
  const needsEdit = thirdPartyModelNeedsEdit(model);
  const showUse = !selected && !needsEdit && model.configured;

  return (
    <li
      className={`runtime-route-models__item${selected ? " runtime-route-models__item--selected" : ""}`}
    >
      <div className="runtime-route-models__info">
        <div className="runtime-route-models__name">{model.name}</div>
        <div className="runtime-route-models__meta">
          {model.provider}
          {" · "}
          <ConnectivityStatus label={model.statusLabel} />
        </div>
      </div>
      {showUse && (
        <button
          type="button"
          className="runtime-route-models__pick"
          disabled={disabled || testing}
          onClick={onSelect}
        >
          {testing ? "Testing…" : "Use"}
        </button>
      )}
      {needsEdit && (
        <button
          type="button"
          className="runtime-route-models__pick"
          disabled={disabled}
          onClick={onEdit}
        >
          Edit
        </button>
      )}
    </li>
  );
}
