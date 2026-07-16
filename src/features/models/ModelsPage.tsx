import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation } from "react-router-dom";

import {
  Button,
  Card,
  Modal,
  PageHeader,
  RefreshButton,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  browseModels,
  cancelModelDownload,
  cancelModelDownloadVerify,
  getModelDownloadStatus,
  getModelsRegistryInfo,
  getModelsVaultPath,
  getModelsVaultStats,
  importModelGguf,
  importModelZip,
  listModels,
  pauseModelDownload,
  removeModel,
  resumeModelDownload,
  retryModelDownloadVerify,
  startModelDownload,
  testModelConnection,
  testModelInference,
  type ModelCatalogEntryDto,
  type ModelDownloadProgressDto,
  type ModelEntryDto,
  type ModelRegistryInfoDto,
  type ModelVaultStatsDto,
} from "@/shared/ipc/models";
import { getRuntimeConfiguration } from "@/shared/ipc/runtime";
import { pickAnyModelImportFile } from "@/shared/ipc/dialog";
import { useToast } from "@/shared/notifications";
import { useRuntimeModelLoading } from "@/shared/hooks/useRuntimeModelLoading";
import { formatBytes } from "@/shared/utils/format";
import { DownloadManagerCard } from "./DownloadManagerCard";
import { ModelRegistrySection } from "./ModelRegistrySection";
import { AddModelModal, type AddModelTab } from "./AddModelModal";
import { loadThirdPartyModelForm, type ThirdPartyModelForm } from "@/shared/ipc/thirdPartyModels";

type ModelsPageLocationState = {
  openAddModel?: boolean;
  openAddModelTab?: AddModelTab;
  editModelId?: string;
};

function isThirdPartyModel(model: ModelEntryDto): boolean {
  return model.format === "api" || model.id.startsWith("remote-");
}

function findLoadedLocalModel(
  models: ModelEntryDto[],
  loadedModelPath: string | null,
): ModelEntryDto | undefined {
  if (!loadedModelPath) return undefined;
  const fileName = loadedModelPath.split(/[/\\]/).pop() ?? "";
  const stem = fileName.replace(/\.gguf$/i, "");
  return models.find(
    (m) =>
      !isThirdPartyModel(m) &&
      (m.name === stem ||
        loadedModelPath.endsWith(`/${m.name}.gguf`) ||
        loadedModelPath.endsWith(`\\${m.name}.gguf`)),
  );
}

export function ModelsPage() {
  const { notify } = useToast();
  const location = useLocation();
  const [backendConnected, setBackendConnected] = useState(false);
  const [installed, setInstalled] = useState<ModelEntryDto[]>([]);
  const [catalog, setCatalog] = useState<ModelCatalogEntryDto[]>([]);
  const [registryInfo, setRegistryInfo] = useState<ModelRegistryInfoDto | null>(null);
  const [vaultPath, setVaultPath] = useState<string>("");
  const [vaultStats, setVaultStats] = useState<ModelVaultStatsDto | null>(null);
  const [importName, setImportName] = useState("");
  const [importPath, setImportPath] = useState("");
  const [importBusy, setImportBusy] = useState<"browse" | "import" | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<ModelDownloadProgressDto | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [busyModelIds, setBusyModelIds] = useState<Set<string>>(() => new Set());
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [addModelOpen, setAddModelOpen] = useState(false);
  const [addModelInitialTab, setAddModelInitialTab] = useState<AddModelTab>("public");
  const [editThirdPartyForm, setEditThirdPartyForm] = useState<ThirdPartyModelForm | null>(null);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [localTestConfirm, setLocalTestConfirm] = useState<{
    target: ModelEntryDto;
    loadedName: string;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const verifyInFlightRef = useRef(false);
  const deepLinkEditRef = useRef<string | null>(null);

  const setModelBusy = useCallback((modelId: string, busy: boolean) => {
    setBusyModelIds((prev) => {
      const next = new Set(prev);
      if (busy) next.add(modelId);
      else next.delete(modelId);
      return next;
    });
  }, []);

  const isModelBusy = useCallback((modelId: string) => busyModelIds.has(modelId), [busyModelIds]);
  const installedNames = useMemo(() => new Set(installed.map((m) => m.name)), [installed]);
  const { modelLoading: runtimeModelLoading, modelTesting: runtimeModelTesting, testingModelId: runtimeTestingModelId } =
    useRuntimeModelLoading(backendConnected);

  useEffect(() => {
    const state = location.state as ModelsPageLocationState | null;
    if (!state) return;

    if (state.openAddModel) {
      if (state.openAddModelTab) {
        setAddModelInitialTab(state.openAddModelTab);
      }
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
        setAddModelInitialTab("third-party");
        setAddModelOpen(true);
      } catch (err) {
        notify(toAppError(err).message, "error");
      } finally {
        setModelBusy(state.editModelId!, false);
      }
    })();
  }, [location.state, backendConnected, notify]);

  const refreshModels = useCallback(async () => {
    const [models, entries, info, stats] = await Promise.all([
      listModels(),
      browseModels(),
      getModelsRegistryInfo(),
      getModelsVaultStats(),
    ]);
    setInstalled(models);
    setCatalog(entries);
    setRegistryInfo(info);
    setVaultStats(stats);
  }, []);

  const applyDownloadStatus = useCallback(
    async (status: Awaited<ReturnType<typeof getModelDownloadStatus>>) => {
      if (status.installed) {
        verifyInFlightRef.current = false;
        setDownloadProgress(null);
        setDownloadingId(null);
        await refreshModels();
        notify(`Installed ${status.installed.name}`, "success");
        return;
      }
      if (status.progress) {
        const downgradedWhileVerifying =
          verifyInFlightRef.current &&
          (status.progress.status === "downloaded" || status.progress.status === "completed");
        if (downgradedWhileVerifying) {
          setDownloadProgress((prev) =>
            prev
              ? { ...prev, status: "verifying", error: null }
              : { ...status.progress!, status: "verifying", error: null },
          );
        } else {
          if (
            status.progress.status === "verifying" ||
            status.progress.status === "downloaded" ||
            status.progress.status === "verify_failed" ||
            status.progress.status === "failed"
          ) {
            verifyInFlightRef.current = status.progress.status === "verifying";
          }
          setDownloadProgress(status.progress);
        }
        if (status.progress.status === "failed" || status.progress.status === "verify_failed") {
          verifyInFlightRef.current = false;
          setDownloadingId(null);
          if (status.progress.status === "failed") {
            setError(status.progress.error ?? "Model download failed");
          }
        } else if (
          status.progress.status === "downloading" ||
          status.progress.status === "paused" ||
          status.progress.status === "verifying" ||
          status.progress.status === "downloaded" ||
          status.progress.status === "completed"
        ) {
          setDownloadingId(status.progress.catalogId);
        } else {
          setDownloadingId(null);
        }
      } else {
        verifyInFlightRef.current = false;
        setDownloadProgress(null);
        setDownloadingId(null);
      }
    },
    [refreshModels, notify],
  );

  const pollDownloadStatus = useCallback(async () => {
    const status = await getModelDownloadStatus();
    await applyDownloadStatus(status);
  }, [applyDownloadStatus]);

  useEffect(() => {
    void import("@/shared/ipc/client").then(({ healthCheck }) =>
      healthCheck()
        .then(() => setBackendConnected(true))
        .catch(() => setBackendConnected(false)),
    );
  }, []);

  useEffect(() => {
    if (!backendConnected) {
      return;
    }
    void refreshModels().catch((err) => setError(toAppError(err).message));
    void getModelsVaultPath()
      .then(setVaultPath)
      .catch(() => undefined);
  }, [backendConnected, refreshModels]);

  const handleRefresh = useCallback(async () => {
    if (!backendConnected || refreshing) return;
    setRefreshing(true);
    setError(null);
    try {
      await Promise.all([
        refreshModels(),
        getModelsVaultPath().then(setVaultPath).catch(() => undefined),
        pollDownloadStatus(),
      ]);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setRefreshing(false);
    }
  }, [backendConnected, refreshing, refreshModels, pollDownloadStatus]);

  useEffect(() => {
    if (!backendConnected) {
      return;
    }

    void pollDownloadStatus().catch((err) => setError(toAppError(err).message));

    const status = downloadProgress?.status;
    if (status === "failed" || status === "verify_failed") {
      return;
    }

    const intervalMs = status === "verifying" ? 500 : status ? 750 : 2000;
    const timer = window.setInterval(() => {
      void pollDownloadStatus().catch((err) => setError(toAppError(err).message));
    }, intervalMs);
    return () => window.clearInterval(timer);
  }, [backendConnected, downloadProgress?.status, pollDownloadStatus]);

  async function handleRetryVerify(catalogId: string) {
    setError(null);
    verifyInFlightRef.current = true;
    setDownloadProgress((prev) =>
      prev ? { ...prev, status: "verifying", error: null } : prev,
    );
    try {
      const status = await retryModelDownloadVerify({ catalogId });
      await applyDownloadStatus(status);
    } catch (err) {
      verifyInFlightRef.current = false;
      setError(toAppError(err).message);
      void pollDownloadStatus().catch(() => undefined);
    }
  }

  async function handleStartVerify(catalogId: string) {
    await handleRetryVerify(catalogId);
  }

  async function handleCancelVerify() {
    setError(null);
    verifyInFlightRef.current = false;
    try {
      const progress = await cancelModelDownloadVerify();
      setDownloadProgress(progress);
      setDownloadingId(progress.catalogId);
    } catch (err) {
      setError(toAppError(err).message);
    }
  }

  async function handleBrowse() {
    setError(null);
    setImportBusy("browse");
    try {
      const path = await pickAnyModelImportFile();
      if (!path) {
        notify("No file selected", "error");
        return;
      }
      setImportPath(path);
      if (!importName.trim()) {
        const base = path.split(/[/\\]/).pop() ?? "";
        const stem = base.replace(/\.(gguf|zip)$/i, "");
        if (stem) {
          setImportName(stem);
        }
      }
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setImportBusy(null);
    }
  }


  async function handleInstall(entry: ModelCatalogEntryDto) {
    // Only one download can run at a time. Ignore clicks while one is active
    // (the button is also disabled in this state) so we never clear the
    // in-flight progress card or trigger an "already active" error.
    if (downloadingId !== null) {
      return;
    }
    setError(null);
    // Clear any previous terminal (failed/completed) download card before retrying.
    setDownloadProgress(null);
    setInstallingId(entry.id);
    try {
      const progress = await startModelDownload({ catalogId: entry.id });
      setDownloadingId(entry.id);
      setDownloadProgress(progress);
      setAddModelOpen(false);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setInstallingId(null);
    }
  }

  async function handleImport() {
    if (!importName.trim()) {
      notify("Display name is required", "error");
      return;
    }
    if (!importPath.trim()) {
      notify("Select a .gguf or .zip file to import", "error");
      return;
    }
    const kind: "gguf" | "zip" = importPath.trim().toLowerCase().endsWith(".zip")
      ? "zip"
      : "gguf";
    setError(null);
    setImportBusy("import");
    try {
      const request = { name: importName.trim(), path: importPath.trim() };
      if (kind === "gguf") {
        await importModelGguf(request);
      } else {
        await importModelZip(request);
      }
      setImportName("");
      setImportPath("");
      await refreshModels();
      setAddModelOpen(false);
      notify("Model imported", "success");
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setImportBusy(null);
    }
  }

  function openAddModel(tab: AddModelTab = "public") {
    setEditThirdPartyForm(null);
    setEditingModelId(null);
    setAddModelInitialTab(tab);
    setAddModelOpen(true);
  }

  async function handleEdit(model: ModelEntryDto) {
    if (!isThirdPartyModel(model)) {
      return;
    }
    setError(null);
    setModelBusy(model.id, true);
    try {
      const form = await loadThirdPartyModelForm(model.id);
      setEditThirdPartyForm(form);
      setEditingModelId(model.id);
      setAddModelInitialTab("third-party");
      setAddModelOpen(true);
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setModelBusy(model.id, false);
    }
  }

  async function handleRemove(modelId: string) {
    if (runtimeModelLoading) {
      const model = installed.find((m) => m.id === modelId);
      if (model && !isThirdPartyModel(model)) return;
    }
    setError(null);
    setModelBusy(modelId, true);
    try {
      await removeModel(modelId);
      await refreshModels();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setModelBusy(modelId, false);
    }
  }

  async function runLocalModelTest(model: ModelEntryDto) {
    setError(null);
    setModelBusy(model.id, true);
    try {
      const result = await testModelInference(model.id);
      notify(`${result.mode}: ${result.sample}`, result.ok ? "success" : "error");
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setModelBusy(model.id, false);
    }
  }

  async function handleTest(model: ModelEntryDto) {
    setError(null);
    try {
      if (isThirdPartyModel(model)) {
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
        } finally {
          setModelBusy(model.id, false);
        }
      } else {
        if (runtimeModelLoading) return;
        const config = await getRuntimeConfiguration();
        const { modelLoaded, loadedModelPath } = config.runtimeStatus;
        const selectedId = config.settings.selectedModelId;
        if (modelLoaded && selectedId && selectedId !== model.id) {
          const loaded = installed.find((m) => m.id === selectedId);
          setLocalTestConfirm({
            target: model,
            loadedName: loaded?.name ?? config.modelName ?? "the loaded model",
          });
          return;
        }
        if (modelLoaded && !selectedId) {
          const loaded = findLoadedLocalModel(installed, loadedModelPath);
          if (loaded && loaded.id !== model.id) {
            setLocalTestConfirm({
              target: model,
              loadedName: loaded.name,
            });
            return;
          }
        }
        await runLocalModelTest(model);
      }
    } catch (err) {
      notify(toAppError(err).message, "error");
    }
  }

  return (
    <div className="page models-page">
      <PageHeader
        title="Models"
        description="Manage local and cloud models for use with AI Runtime"
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
            Connect to the Tauri backend to manage local models and downloads.
          </p>
        </Card>
      ) : null}

      <section className="models-page__overview" aria-label="Vault overview">
        <Card className="detail-section models-page__stats">
          <h2 className="detail-section__title">Vault summary</h2>
          {vaultStats ? (
            <div className="detail-summary-grid detail-summary-grid--metrics">
              <div className="summary-stat">
                <span className="summary-stat__label">Registered</span>
                <span className="summary-stat__value">{vaultStats.registeredCount}</span>
              </div>
              <div className="summary-stat">
                <span className="summary-stat__label">Installed local</span>
                <span className="summary-stat__value">{vaultStats.installedLocalCount}</span>
              </div>
              <div className="summary-stat summary-stat--success">
                <span className="summary-stat__label">Disk usage</span>
                <span className="summary-stat__value">{vaultStats.installedGb.toFixed(2)} GB</span>
              </div>
              <div className="summary-stat">
                <span className="summary-stat__label">Bytes on disk</span>
                <span className="summary-stat__value summary-stat__value--sm">
                  {formatBytes(vaultStats.installedBytes)}
                </span>
              </div>
            </div>
          ) : (
            <p className="text-muted text-sm">Vault statistics load when the backend is connected.</p>
          )}
        </Card>

        <Card className="detail-section models-page__meta">
          <h2 className="detail-section__title">Storage and registry</h2>
          <dl className="models-page__meta-list">
            <div>
              <dt>Vault path</dt>
              <dd className="mono">{vaultPath || "Unavailable"}</dd>
            </div>
            {registryInfo ? (
              <>
                <div>
                  <dt>Registry entries</dt>
                  <dd>
                    {registryInfo.validModels} valid / {registryInfo.totalModels} total
                    {registryInfo.invalidModels > 0
                      ? ` (${registryInfo.invalidModels} invalid)`
                      : ""}
                  </dd>
                </div>
                <div>
                  <dt>Catalog source</dt>
                  <dd>{registryInfo.remoteMerged ? "Online merge active" : "Offline bundle"}</dd>
                </div>
                {registryInfo.sourcePath ? (
                  <div className="models-page__meta-wide">
                    <dt>Source path</dt>
                    <dd className="mono">{registryInfo.sourcePath}</dd>
                  </div>
                ) : null}
              </>
            ) : (
              <div>
                <dt>Registry</dt>
                <dd className="text-muted">Loading registry metadata…</dd>
              </div>
            )}
          </dl>
        </Card>
      </section>

      {downloadProgress ? (
        <section className="models-page__download" aria-label="Active download">
          <DownloadManagerCard
            progress={downloadProgress}
            backendConnected={backendConnected}
            onPause={() =>
              void pauseModelDownload()
                .then(setDownloadProgress)
                .catch((err) => setError(toAppError(err).message))
            }
            onResume={() =>
              void resumeModelDownload()
                .then(setDownloadProgress)
                .catch((err) => setError(toAppError(err).message))
            }
            onCancel={() =>
              void cancelModelDownload()
                .then(() => {
                  setDownloadProgress(null);
                  setDownloadingId(null);
                })
                .catch((err) => setError(toAppError(err).message))
            }
            onRetryVerify={() => void handleRetryVerify(downloadProgress.catalogId)}
            onCancelVerify={() => void handleCancelVerify()}
            onStartVerify={() => void handleStartVerify(downloadProgress.catalogId)}
          />
        </section>
      ) : null}

      <section className="models-page__primary" aria-label="Model registry">
        <ModelRegistrySection
          models={installed}
          isModelBusy={isModelBusy}
          runtimeModelLoading={runtimeModelLoading}
          runtimeModelTesting={runtimeModelTesting}
          runtimeTestingModelId={runtimeTestingModelId}
          onTest={(model) => void handleTest(model)}
          onEdit={(model) => void handleEdit(model)}
          onRemove={(modelId) => void handleRemove(modelId)}
        />
      </section>

      <Modal
        open={localTestConfirm !== null}
        title="Switch model in AI Runtime?"
        onClose={() => setLocalTestConfirm(null)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setLocalTestConfirm(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              disabled={runtimeModelLoading}
              onClick={() => {
                const target = localTestConfirm?.target;
                setLocalTestConfirm(null);
                if (target) void runLocalModelTest(target);
              }}
            >
              Continue
            </Button>
          </>
        }
      >
        {localTestConfirm ? (
          <p>
            <strong>{localTestConfirm.loadedName}</strong> is active in AI Runtime. To test{" "}
            <strong>{localTestConfirm.target.name}</strong>, the runtime will unload the current
            model and load this one first. Continue?
          </p>
        ) : null}
      </Modal>

      <AddModelModal
        open={addModelOpen}
        initialTab={addModelInitialTab}
        initialThirdPartyForm={editThirdPartyForm}
        editingModelId={editingModelId}
        modalTitle={editThirdPartyForm ? "Edit Model" : "Add Model"}
        onClose={() => {
          setAddModelOpen(false);
          setEditThirdPartyForm(null);
          setEditingModelId(null);
        }}
        backendConnected={backendConnected}
        catalog={catalog}
        installedNames={installedNames}
        downloadingId={downloadingId}
        installingId={installingId}
        importName={importName}
        importPath={importPath}
        importBusy={importBusy}
        onImportNameChange={setImportName}
        onImportPathChange={setImportPath}
        onInstall={(entry) => void handleInstall(entry)}
        onBrowseImport={() => void handleBrowse()}
        onImport={() => void handleImport()}
        onThirdPartySaved={() => {
          setEditThirdPartyForm(null);
          setEditingModelId(null);
          void refreshModels();
        }}
      />
    </div>
  );
}
