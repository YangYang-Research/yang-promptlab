import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";

import {
  Button,
  Card,
  ConnectivityStatus,
  connectivityStatusVariant,
  EmptyState,
  Modal,
  PageHeader,
  PageLoadingSkeleton,
  RefreshButton,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { recordLocalActivity } from "@/shared/activity/localActivity";
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import { useRuntimeModelLoading } from "@/shared/hooks/useRuntimeModelLoading";
import { isYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import {
  getRuntimeHardware,
  getRuntimeHealth,
  getRuntimeLogs,
  loadRuntimeModel,
  refreshRuntimeHardware,
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

import { RegistryProviderIcon } from "@/features/models/ProviderLogo";
import { RuntimeTrafficChart } from "@/features/runtime/RuntimeTrafficChart";

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

/** Soft recommendation for Local runtime; does not block switching. */
const LOCAL_RUNTIME_MIN_RAM_BYTES = 8 * 1024 * 1024 * 1024;
const LOCAL_RUNTIME_MIN_DISK_BYTES = 10 * 1024 * 1024 * 1024;

const RUNTIME_MODE_OPTIONS: RuntimeModeOption[] = [
  {
    route: "third_party",
    title: "Third-party Providers",
    badge: "Remote",
    summary: "Route AI requests to a cloud provider you register in Models.",
    highlights: [
      "OpenAI, Anthropic, AWS Bedrock, Google, Azure, and custom endpoints",
      "API keys stored on this device (OS keychain when available)",
      "Traffic goes directly to the provider — not through PromptLab servers",
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
        body: "All inference stays on-device. PromptLab does not send prompts or model outputs to third-party services in this mode.",
      },
      {
        label: "Hardware",
        body: "8 GB RAM and at least 10 GB free disk for the runtime plus a compact Q4 model. Larger models need 16 GB+ RAM or Apple Silicon / CUDA acceleration.",
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
    <Card className="runtime-mode-picker detail-section">
      <h2 className="detail-section__title runtime-mode-picker__title">Choose AI Runtime Mode</h2>
      <p className="runtime-mode-picker__lead">
        Pick how PromptLab runs AI features. You can change this later after configuration.
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
    </Card>
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
  const [localRamWarning, setLocalRamWarning] = useState<string | null>(null);

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
    if (!backendConnected || refreshing || testingModelId !== null) return;
    setError(null);
    setRefreshing(true);
    try {
      await refreshAll();

      if (mode === "third_party") {
        const modelId = settings?.selectedModelId;
        if (modelId) {
          setTestingModelId(modelId);
          try {
            const result = await setRoute("third_party", modelId);
            if (
              result?.settings.connectivityTestOk === false &&
              result.settings.connectivityTestDetail
            ) {
              notify(result.settings.connectivityTestDetail, "error");
            }
          } finally {
            setTestingModelId(null);
          }
        }
      } else if (mode === "local") {
        await getRuntimeHealth();
        await refreshAll();
      }
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setRefreshing(false);
    }
  }, [
    backendConnected,
    refreshing,
    testingModelId,
    refreshAll,
    mode,
    settings?.selectedModelId,
    setRoute,
    notify,
  ]);

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

  useEffect(() => {
    if (!backendConnected || mode !== "local") {
      if (mode !== "local") setLocalRamWarning(null);
      return;
    }
    void evaluateLocalHardwareWarning().then((warning) => {
      setLocalRamWarning(warning);
    });
    // Only re-check when entering/staying on local after connect.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentional gate on mode/backend
  }, [backendConnected, mode]);

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

  async function evaluateLocalHardwareWarning(): Promise<string | null> {
    let profile: RuntimeHardwareDto | null = hardware;
    try {
      profile = await refreshRuntimeHardware();
      setHardware(profile);
    } catch {
      try {
        profile = await getRuntimeHardware();
        setHardware(profile);
      } catch {
        return null;
      }
    }
    if (!profile) return null;

    const parts: string[] = [];
    if (profile.ramBytes < LOCAL_RUNTIME_MIN_RAM_BYTES) {
      const detectedGb = (profile.ramBytes / (1024 * 1024 * 1024)).toFixed(1);
      parts.push(`RAM ${detectedGb} GB (need ≥ 8 GB)`);
    }

    const diskFree = profile.diskFreeBytes;
    if (diskFree != null && diskFree <= LOCAL_RUNTIME_MIN_DISK_BYTES) {
      const detectedGb = (diskFree / (1024 * 1024 * 1024)).toFixed(1);
      parts.push(`disk free ${detectedGb} GB (need > 10 GB)`);
    }

    if (parts.length === 0) return null;
    return `Local runtime: ${parts.join("; ")}. You can continue, but performance or downloads may fail.`;
  }

  async function warnIfLocalHardwareBelowRecommendation() {
    const warning = await evaluateLocalHardwareWarning();
    setLocalRamWarning(warning);
    if (warning) {
      notify(warning, "warning");
    }
  }

  async function handleModeChange(route: AiInferenceRoute) {
    if (routeBusy || busy !== null || engineInitializing) return;
    if (modelLoadInProgress) return;
    if (route === "third_party" && (runtimeModelLoading || modelRuntimeAction?.action === "load")) {
      return;
    }
    try {
      if (route === "local") {
        if (mode !== "local") {
          await warnIfLocalHardwareBelowRecommendation();
        }
      } else {
        setLocalRamWarning(null);
      }
      await setRoute(route);
      recordLocalActivity({
        type: "runtime",
        message:
          route === "local"
            ? "Selected AI Runtime mode: Local"
            : "Selected AI Runtime mode: Third-party",
      });
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
      const modelName =
        (settings?.thirdPartyModels ?? []).find((model) => model.id === modelId)?.name ?? modelId;
      recordLocalActivity({
        type: "runtime",
        message: `Selected AI Runtime model: ${modelName}`,
      });
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
      const modelName =
        (settings?.localModels ?? []).find((model) => model.id === modelId)?.name ?? modelId;
      recordLocalActivity({
        type: "runtime",
        message: `Used model: ${modelName}`,
      });
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
  const connectionDotVariant = testingModelId
    ? null
    : connectivityStatusVariant(configuration?.connectivity);

  return (
    <div className={`page runtime-page${showModePicker ? " page--runtime-setup" : ""}`}>
      <PageHeader
        title="AI Runtime"
        description={
          showModePicker
            ? "Choose a cloud or local runtime for Yazg Agent"
            : "Manage the AI Runtime for Yazg Agent"
        }
        actions={
          backendConnected ? (
            <div className="page-header__actions-row">
              <RefreshButton
                loading={refreshing || configLoading || testingModelId !== null}
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

      {error ? (
        <div className="runtime-page__alert" role="alert">
          {error}
        </div>
      ) : null}

      {localRamWarning ? (
        <div className="runtime-page__alert runtime-page__alert--warning" role="status">
          {localRamWarning}
        </div>
      ) : null}

      {!backendConnected ? (
        <Card className="runtime-page__banner detail-section">
          <p className="runtime-page__banner-text">
            Connect to the Tauri backend to manage the AI runtime.
          </p>
        </Card>
      ) : null}

      {configLoading && !configuration && backendConnected ? <PageLoadingSkeleton /> : null}

      {configError && !configuration && backendConnected && !configLoading ? (
        <div className="runtime-page__alert" role="alert">
          {configError}
        </div>
      ) : null}

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
              <section className="runtime-page__overview" aria-label="Runtime overview">
                <Card className="detail-section runtime-page__status">
                  <h2 className="detail-section__title">Status</h2>
                  <div className="detail-summary-grid detail-summary-grid--metrics">
                    <div className="summary-stat">
                      <span className="summary-stat__label">Runtime status</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {configuration?.statusLabel ? (
                          <ConnectivityStatus label={configuration.statusLabel} />
                        ) : (
                          "N/A"
                        )}
                      </span>
                    </div>
                    <div className="summary-stat">
                      <span className="summary-stat__label">Provider</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {configuration?.provider ? (
                          <RegistryProviderIcon provider={configuration.provider} />
                        ) : (
                          "N/A"
                        )}
                      </span>
                    </div>
                    <div className="summary-stat">
                      <span className="summary-stat__label">Model ID</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {configuration?.modelName ?? settings?.selectedModelName ?? "N/A"}
                      </span>
                    </div>
                    <div className="summary-stat">
                      <span className="summary-stat__label">Yazg Agent</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {testingModelId ? (
                          "Checking…"
                        ) : (
                          <ConnectivityStatus
                            label={isYazgAgentLive(configuration) ? "Live" : "Offline"}
                          />
                        )}
                      </span>
                    </div>
                  </div>
                </Card>

                <Card className="detail-section runtime-page__meta">
                  <h2 className="detail-section__title runtime-page__connection-title">
                    Connection
                    {connectionDotVariant ? (
                      <span
                        className={`connectivity-status__dot connectivity-status__dot--${connectionDotVariant}`}
                        aria-hidden
                      />
                    ) : null}
                  </h2>
                  <dl className="runtime-page__meta-list">
                    <div>
                      <dt>Connectivity</dt>
                      <dd>
                        {testingModelId
                          ? "Testing…"
                          : configuration?.connectivity ?? "N/A"}
                      </dd>
                    </div>
                    <div>
                      <dt>Last health check</dt>
                      <dd>
                        {testingModelId
                          ? "Running connection test…"
                          : configuration?.lastHealthCheck ?? "Not checked"}
                      </dd>
                    </div>
                  </dl>
                </Card>
              </section>

              <Card className="detail-section runtime-page__traffic">
                <h2 className="detail-section__title">Traffic monitor</h2>
                <RuntimeTrafficChart
                  enabled={backendConnected && mode === "third_party"}
                  defaultRangeId="1m"
                />
              </Card>

              <section className="runtime-page__primary" aria-label="Registered models">
                <Card className="detail-section">
                  <div className="detail-section__header">
                    <div>
                      <h2 className="detail-section__title">Registered models</h2>
                      <p className="detail-section__hint">
                        {thirdPartyModels.length === 0
                          ? "Add a cloud provider model to route AI requests."
                          : `${thirdPartyModels.length} registered model${thirdPartyModels.length === 1 ? "" : "s"}`}
                      </p>
                    </div>
                    <div className="detail-section__header-actions">
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() =>
                          navigate("/models", {
                            state: { openAddModel: true, openAddModelTab: "third-party" },
                          })
                        }
                      >
                        Add Model
                      </Button>
                    </div>
                  </div>

                  {thirdPartyModels.length > 0 ? (
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
                  ) : (
                    <EmptyState
                      title="No remote models yet"
                      description="Register an OpenAI, Anthropic, or custom endpoint provider from Models."
                    />
                  )}
                </Card>
              </section>
            </>
          )}

          {mode === "local" && (
            <>
              <section className="runtime-page__overview" aria-label="Runtime overview">
                <Card className="detail-section runtime-page__status">
                  <h2 className="detail-section__title">Status</h2>
                  <div className="detail-summary-grid detail-summary-grid--metrics">
                    <div className="summary-stat">
                      <span className="summary-stat__label">Runtime status</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        <span className="connectivity-status">
                          {localRuntimeStatusLabel}
                          <span
                            className={`connectivity-status__dot connectivity-status__dot--${
                              connectivityStatusVariant(localRuntimeStatusLabel) ?? "warning"
                            }`}
                            aria-hidden
                          />
                        </span>
                      </span>
                    </div>
                    <div className="summary-stat">
                      <span className="summary-stat__label">Current runtime</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {configuration?.runtimeName ?? "N/A"}
                      </span>
                    </div>
                    <div className="summary-stat">
                      <span className="summary-stat__label">Model ID</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {status?.modelLoaded
                          ? (configuration?.modelName ?? status?.loadedModelPath ?? "N/A")
                          : "N/A"}
                      </span>
                    </div>
                    <div className="summary-stat">
                      <span className="summary-stat__label">Yazg Agent</span>
                      <span className="summary-stat__value summary-stat__value--sm">
                        {modelLoadInProgress ? (
                          "Checking…"
                        ) : (
                          <ConnectivityStatus
                            label={isYazgAgentLive(configuration) ? "Live" : "Offline"}
                          />
                        )}
                      </span>
                    </div>
                  </div>
                </Card>

                <Card className="detail-section runtime-page__hardware">
                  <h2 className="detail-section__title">Hardware</h2>
                  {hardware ? (
                    <dl className="runtime-page__meta-list">
                      <div>
                        <dt>CPU</dt>
                        <dd>
                          {hardware.cpu} ({hardware.cpuCores} cores)
                        </dd>
                      </div>
                      <div>
                        <dt>RAM</dt>
                        <dd>{formatBytes(hardware.ramBytes)}</dd>
                      </div>
                      <div>
                        <dt>Free disk</dt>
                        <dd>
                          {hardware.diskFreeBytes != null
                            ? formatBytes(hardware.diskFreeBytes)
                            : "N/A"}
                        </dd>
                      </div>
                      <div>
                        <dt>GPU</dt>
                        <dd>{hardware.gpuName ?? "None detected"}</dd>
                      </div>
                      <div>
                        <dt>Acceleration</dt>
                        <dd>
                          {[
                            hardware.cuda ? "CUDA" : null,
                            hardware.metal ? "Metal" : null,
                          ]
                            .filter(Boolean)
                            .join(", ") || "CPU only"}
                        </dd>
                      </div>
                    </dl>
                  ) : (
                    <p className="text-muted text-sm">
                      Hardware profile not available yet. Reinitialize the engine to detect this
                      machine.
                    </p>
                  )}
                </Card>
              </section>

              <section className="runtime-page__primary" aria-label="Installed models">
                <Card className="detail-section">
                  <div className="detail-section__header">
                    <div>
                      <h2 className="detail-section__title">Installed models</h2>
                      <p className="detail-section__hint">
                        {localModels.length === 0
                          ? "Install a GGUF model, then load it into the local runtime."
                          : `${localModels.length} local model${localModels.length === 1 ? "" : "s"} — load one to start inference`}
                      </p>
                    </div>
                    <div className="detail-section__header-actions">
                      <Button
                        variant="ghost"
                        size="sm"
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
                        size="sm"
                        disabled={disabled}
                        onClick={() =>
                          void runAction("Benchmark", async () => {
                            setBenchmark(await runRuntimeBenchmark());
                          })
                        }
                      >
                        {busy === "Benchmark" ? "Running…" : "Benchmark"}
                      </Button>
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() =>
                          navigate("/models", {
                            state: { openAddModel: true, openAddModelTab: "public" },
                          })
                        }
                      >
                        Add Model
                      </Button>
                    </div>
                  </div>

                  {localModels.length === 0 ? (
                    <EmptyState
                      title="No local models yet"
                      description="Download a catalog model or import a GGUF file from Models."
                    />
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

                  {benchmark ? (
                    <dl className="runtime-kv-grid runtime-page__benchmark">
                      <div>
                        <dt>Benchmark latency</dt>
                        <dd>{benchmark.latencyMs} ms</dd>
                      </div>
                      <div>
                        <dt>Throughput</dt>
                        <dd>{benchmark.tokensPerSec.toFixed(1)} tok/s</dd>
                      </div>
                    </dl>
                  ) : null}
                </Card>
              </section>

              <section className="runtime-page__engine" aria-label="Inference engine">
                <Card className="detail-section">
                  <div className="detail-section__header">
                    <div>
                      <h2 className="detail-section__title">Inference engine</h2>
                      <p className="detail-section__hint">
                        Embedded libllama runs in-process — no separate runtime binary to install.
                      </p>
                    </div>
                  </div>

                  <dl className="runtime-kv-grid">
                    <div>
                      <dt>Engine</dt>
                      <dd>
                        {status?.recommendedRuntime ?? "Reinitialize engine to profile hardware"}
                      </dd>
                    </div>
                    <div>
                      <dt>Version</dt>
                      <dd>{configuration?.runtimeVersion ?? status?.runtimeVersion ?? "N/A"}</dd>
                    </div>
                    <div>
                      <dt>Inference</dt>
                      <dd>
                        <ConnectivityStatus label={inferenceLabel} />
                      </dd>
                    </div>
                    <div>
                      <dt>Status</dt>
                      <dd>{status?.message ?? "N/A"}</dd>
                    </div>
                  </dl>

                  {engineInitializing ? (
                    <RuntimeEngineProgress
                      progress={engineInitProgress}
                      initializing={engineInitializing}
                      error={error}
                    />
                  ) : null}

                  <div className="model-card__actions">
                    <Button disabled={disabled} onClick={() => void runEngineReinitialize()}>
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

                  {showLogs ? (
                    <div className="runtime-log-card runtime-log-card--inline">
                      {logs.length === 0 ? (
                        <p className="text-muted">No runtime logs yet.</p>
                      ) : (
                        <ul className="runtime-log-list">
                          {logs.map((entry, index) => (
                            <li
                              key={`${entry.timestamp}-${index}`}
                              className="runtime-log-list__item"
                            >
                              <span className="runtime-log-list__time">{entry.timestamp}</span>
                              <span
                                className={`runtime-log-list__level runtime-log-list__level--${entry.level}`}
                              >
                                {entry.level}
                              </span>
                              <span>{entry.message}</span>
                            </li>
                          ))}
                        </ul>
                      )}
                    </div>
                  ) : null}
                </Card>
              </section>
            </>
          )}

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
