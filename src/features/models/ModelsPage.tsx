import { useCallback, useEffect, useState } from "react";
import { useLocation } from "react-router-dom";

import {
  Button,
  Card,
  PageHeader,
  RefreshButton,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { recordLocalActivity } from "@/shared/activity/localActivity";
import {
  listModels,
  removeModel,
  testModelConnection,
  type ModelEntryDto,
} from "@/shared/ipc/models";
import { getRuntimeTokenUsage, type TokenUsageSnapshot } from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";
import { ModelRegistrySection } from "./ModelRegistrySection";
import { AddModelModal, type AddModelTab } from "./AddModelModal";
import { loadThirdPartyModelForm, type ThirdPartyModelForm } from "@/shared/ipc/thirdPartyModels";

type ModelsPageLocationState = {
  openAddModel?: boolean;
  openAddModelTab?: AddModelTab;
  editModelId?: string;
};

function isThirdPartyModel(model: ModelEntryDto): boolean {
  return (
    model.format === "api" ||
    model.id.startsWith("remote-") ||
    model.provider.toLowerCase() === "ollama"
  );
}

function formatTokens(value: number): string {
  return new Intl.NumberFormat().format(value);
}

export function ModelsPage() {
  const { notify } = useToast();
  const location = useLocation();
  const [backendConnected, setBackendConnected] = useState(false);
  const [installed, setInstalled] = useState<ModelEntryDto[]>([]);
  const [tokenUsage, setTokenUsage] = useState<TokenUsageSnapshot | null>(null);
  const [busyModelIds, setBusyModelIds] = useState<Set<string>>(() => new Set());
  const [addModelOpen, setAddModelOpen] = useState(false);
  const [editThirdPartyForm, setEditThirdPartyForm] = useState<ThirdPartyModelForm | null>(null);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const deepLinkEditRef = useState<{ current: string | null }>({ current: null })[0];

  const setModelBusy = useCallback((modelId: string, busy: boolean) => {
    setBusyModelIds((prev) => {
      const next = new Set(prev);
      if (busy) next.add(modelId);
      else next.delete(modelId);
      return next;
    });
  }, []);

  const isModelBusy = useCallback((modelId: string) => busyModelIds.has(modelId), [busyModelIds]);

  useEffect(() => {
    const state = location.state as ModelsPageLocationState | null;
    if (!state) return;

    if (state.openAddModel) {
      setEditThirdPartyForm(null);
      setEditingModelId(null);
      setAddModelOpen(true);
    }

    if (!state.editModelId || !backendConnected) return;
    if (deepLinkEditRef.current === state.editModelId) return;
    deepLinkEditRef.current = state.editModelId;

    void (async () => {
      setError(null);
      setModelBusy(state.editModelId!, true);
      try {
        const form = await loadThirdPartyModelForm(state.editModelId!);
        setEditThirdPartyForm(form);
        setEditingModelId(state.editModelId!);
        setAddModelOpen(true);
      } catch (err) {
        notify(toAppError(err).message, "error");
      } finally {
        setModelBusy(state.editModelId!, false);
      }
    })();
  }, [location.state, backendConnected, notify, setModelBusy, deepLinkEditRef]);

  const refreshModels = useCallback(async () => {
    const settled = await Promise.allSettled([listModels(), getRuntimeTokenUsage()]);
    const [modelsResult, usageResult] = settled;

    if (usageResult.status === "fulfilled") {
      setTokenUsage(usageResult.value);
    } else {
      setTokenUsage(null);
    }

    if (modelsResult.status === "fulfilled") {
      setInstalled(modelsResult.value);
      return;
    }

    throw modelsResult.reason;
  }, []);

  useEffect(() => {
    void import("@/shared/ipc/client").then(({ healthCheck }) =>
      healthCheck()
        .then(() => setBackendConnected(true))
        .catch(() => setBackendConnected(false)),
    );
  }, []);

  useEffect(() => {
    if (!backendConnected) return;
    void refreshModels().catch((err) => setError(toAppError(err).message));
  }, [backendConnected, refreshModels]);

  const handleRefresh = useCallback(async () => {
    if (!backendConnected || refreshing) return;
    setRefreshing(true);
    setError(null);
    try {
      await refreshModels();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setRefreshing(false);
    }
  }, [backendConnected, refreshing, refreshModels]);

  function openAddModel() {
    setEditThirdPartyForm(null);
    setEditingModelId(null);
    setAddModelOpen(true);
  }

  async function handleEdit(model: ModelEntryDto) {
    if (!isThirdPartyModel(model)) return;
    setError(null);
    setModelBusy(model.id, true);
    try {
      const form = await loadThirdPartyModelForm(model.id);
      setEditThirdPartyForm(form);
      setEditingModelId(model.id);
      setAddModelOpen(true);
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setModelBusy(model.id, false);
    }
  }

  async function handleRemove(modelId: string) {
    setError(null);
    setModelBusy(modelId, true);
    try {
      const removed = await removeModel(modelId);
      await refreshModels();
      recordLocalActivity({
        type: "model",
        message: `Removed model: ${removed.name}`,
      }).catch(() => undefined);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setModelBusy(modelId, false);
    }
  }

  async function handleTest(model: ModelEntryDto) {
    setError(null);
    if (!isThirdPartyModel(model)) {
      notify(
        "Local GGUF models are no longer supported — configure a remote provider instead",
        "error",
      );
      return;
    }
    setModelBusy(model.id, true);
    try {
      const result = await testModelConnection(model.id);
      const latency = result.latencyMs > 0 ? ` (${result.latencyMs} ms)` : "";
      const label = `${result.provider} / ${result.model}`;
      if (result.ok) {
        notify(`Connection Successful — ${label}${latency}`, "success");
      } else {
        notify(`Connection Failed — ${label}: ${result.message}`, "error");
      }
      await refreshModels();
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setModelBusy(model.id, false);
    }
  }

  const remoteModels = installed.filter(isThirdPartyModel);

  return (
    <div className="page models-page">
      <PageHeader
        title="Models"
        description="Configure remote AI providers for use with AI Runtime"
        actions={
          <>
            <RefreshButton
              loading={refreshing}
              error={error}
              disabled={!backendConnected}
              onClick={() => void handleRefresh()}
            />
            <Button variant="primary" disabled={!backendConnected} onClick={() => openAddModel()}>
              Add Model
            </Button>
          </>
        }
      />

      {error ? (
        <div className="models-page__alert" role="alert">
          {error}
        </div>
      ) : null}

      {!backendConnected ? (
        <Card className="models-page__banner detail-section">
          <p className="models-page__banner-text">
            Connect to the Tauri backend to manage remote AI providers.
          </p>
        </Card>
      ) : null}

      <section className="models-page__overview" aria-label="Models overview">
        <Card className="detail-section models-page__stats">
          <h2 className="detail-section__title">Provider summary</h2>
          <div className="detail-summary-grid models-page__stats-grid">
            <div className="summary-stat">
              <span className="summary-stat__label">Registered</span>
              <span className="summary-stat__value">{remoteModels.length}</span>
            </div>
            <div className="summary-stat">
              <span className="summary-stat__label">Remote providers</span>
              <span className="summary-stat__value">
                {new Set(remoteModels.map((model) => model.provider.toLowerCase())).size}
              </span>
            </div>
            <div className="summary-stat">
              <span className="summary-stat__label">Input tokens</span>
              <span className="summary-stat__value summary-stat__value--sm">
                {backendConnected ? formatTokens(tokenUsage?.totalInputTokens ?? 0) : "—"}
              </span>
            </div>
            <div className="summary-stat">
              <span className="summary-stat__label">Output tokens</span>
              <span className="summary-stat__value summary-stat__value--sm">
                {backendConnected ? formatTokens(tokenUsage?.totalOutputTokens ?? 0) : "—"}
              </span>
            </div>
          </div>
        </Card>
      </section>

      <section className="models-page__primary" aria-label="Model registry">
        <ModelRegistrySection
          models={remoteModels}
          isModelBusy={isModelBusy}
          runtimeModelLoading={false}
          runtimeModelTesting={false}
          runtimeTestingModelId={null}
          onTest={(model) => void handleTest(model)}
          onEdit={(model) => void handleEdit(model)}
          onRemove={(modelId) => void handleRemove(modelId)}
        />
      </section>

      <AddModelModal
        open={addModelOpen}
        initialThirdPartyForm={editThirdPartyForm}
        editingModelId={editingModelId}
        modalTitle={editThirdPartyForm ? "Edit Model" : "Add Model"}
        onClose={() => {
          setAddModelOpen(false);
          setEditThirdPartyForm(null);
          setEditingModelId(null);
        }}
        backendConnected={backendConnected}
        onThirdPartySaved={() => {
          setEditThirdPartyForm(null);
          setEditingModelId(null);
          void refreshModels();
        }}
      />
    </div>
  );
}
