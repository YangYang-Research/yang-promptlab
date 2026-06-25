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
  const [busyModelId, setBusyModelId] = useState<string | null>(null);
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

  const installedNames = useMemo(() => new Set(installed.map((m) => m.name)), [installed]);
  const { modelLoading: runtimeModelLoading } = useRuntimeModelLoading(backendConnected);

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
      setBusyModelId(state.editModelId!);
      try {
        const form = await loadThirdPartyModelForm(state.editModelId!);
        setEditThirdPartyForm(form);
        setEditingModelId(state.editModelId!);
        setAddModelInitialTab("third-party");
        setAddModelOpen(true);
      } catch (err) {
        notify(toAppError(err).message, "error");
      } finally {
        setBusyModelId(null);
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
    setBusyModelId(model.id);
    try {
      const form = await loadThirdPartyModelForm(model.id);
      setEditThirdPartyForm(form);
      setEditingModelId(model.id);
      setAddModelInitialTab("third-party");
      setAddModelOpen(true);
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setBusyModelId(null);
    }
  }

  async function handleRemove(modelId: string) {
    if (runtimeModelLoading) {
      const model = installed.find((m) => m.id === modelId);
      if (model && !isThirdPartyModel(model)) return;
    }
    setError(null);
    setBusyModelId(modelId);
    try {
      await removeModel(modelId);
      await refreshModels();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusyModelId(null);
    }
  }

  async function runLocalModelTest(model: ModelEntryDto) {
    setError(null);
    setBusyModelId(model.id);
    try {
      const result = await testModelInference(model.id);
      notify(`${result.mode}: ${result.sample}`, result.ok ? "success" : "error");
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setBusyModelId(null);
    }
  }

  async function handleTest(model: ModelEntryDto) {
    setError(null);
    try {
      if (isThirdPartyModel(model)) {
        setBusyModelId(model.id);
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
          setBusyModelId(null);
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
      setBusyModelId(null);
    }
  }

  return (
    <div className="page">
      <PageHeader
        title="Models"
        description="Manage local GGUF models, cloud providers, and imports"
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

      {!backendConnected && (
        <p className="text-muted">Connect to the Tauri backend to manage local models.</p>
      )}

      {vaultPath && (
        <p className="text-muted text-sm model-vault-path">
          Model vault: <span className="mono">{vaultPath}</span>
        </p>
      )}

      {backendConnected && vaultStats && (
        <div className="models-summary-grid">
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Registered models</span>
            <strong className="models-summary-card__value">{vaultStats.registeredCount}</strong>
            <p className="text-muted text-sm">Local catalog, cloud, and import</p>
          </Card>
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Installed models</span>
            <strong className="models-summary-card__value">{vaultStats.installedLocalCount}</strong>
            <p className="text-muted text-sm">Local catalog and file import</p>
          </Card>
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Installed size</span>
            <strong className="models-summary-card__value">
              {vaultStats.installedGb.toFixed(2)} GB
            </strong>
            <p className="text-muted text-sm">{formatBytes(vaultStats.installedBytes)}</p>
          </Card>
        </div>
      )}

      {registryInfo && (
        <p className="text-muted text-sm">
          Registry: {registryInfo.validModels} valid / {registryInfo.totalModels} total
          {registryInfo.invalidModels > 0
            ? ` · ${registryInfo.invalidModels} invalid`
            : ""}
          {registryInfo.remoteMerged ? " · online merge active" : " · offline"}
          {registryInfo.sourcePath ? (
            <>
              {" "}
              · <span className="mono">{registryInfo.sourcePath}</span>
            </>
          ) : null}
        </p>
      )}

      {downloadProgress && (
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
          onRetryVerify={() =>
            void handleRetryVerify(downloadProgress.catalogId)
          }
          onCancelVerify={() => void handleCancelVerify()}
          onStartVerify={() =>
            void handleStartVerify(downloadProgress.catalogId)
          }
        />
      )}

      <ModelRegistrySection
        models={installed}
        busyModelId={busyModelId}
        runtimeModelLoading={runtimeModelLoading}
        onTest={(model) => void handleTest(model)}
        onEdit={(model) => void handleEdit(model)}
        onRemove={(modelId) => void handleRemove(modelId)}
      />

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
              disabled={busyModelId !== null}
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

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
