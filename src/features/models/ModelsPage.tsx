import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  Button,
  Card,
  PageHeader,
  StatusBadge,
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
  testModelInference,
  verifyModel,
  type ModelCatalogEntryDto,
  type ModelDownloadProgressDto,
  type ModelEntryDto,
  type ModelRegistryInfoDto,
  type ModelVaultStatsDto,
} from "@/shared/ipc/models";
import { pickAnyModelImportFile } from "@/shared/ipc/dialog";
import { useToast } from "@/shared/notifications";
import { formatBytes } from "@/shared/utils/format";
import { DownloadManagerCard } from "./DownloadManagerCard";
import { AddModelModal } from "./AddModelModal";

export function ModelsPage() {
  const { notify } = useToast();
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
  const [modelTestMessage, setModelTestMessage] = useState<string | null>(null);
  const [addModelOpen, setAddModelOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const verifyInFlightRef = useRef(false);

  const installedNames = useMemo(() => new Set(installed.map((m) => m.name)), [installed]);

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

  async function handleRemove(modelId: string) {
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

  async function handleVerify(modelId: string) {
    setError(null);
    setBusyModelId(modelId);
    try {
      const result = await verifyModel(modelId);
      setModelTestMessage(
        result.valid ? "Model verified successfully" : "Verification failed",
      );
      await refreshModels();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusyModelId(null);
    }
  }

  async function handleRunInference(modelId: string) {
    setError(null);
    setBusyModelId(modelId);
    try {
      const result = await testModelInference(modelId);
      setModelTestMessage(`${result.mode}: ${result.sample}`);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusyModelId(null);
    }
  }

  return (
    <div className="page">
      <PageHeader
        title="Models"
        description="Manage local GGUF models, public catalog, and third-party cloud providers"
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
            <p className="text-muted text-sm">Public, third-party, and import</p>
          </Card>
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Installed models</span>
            <strong className="models-summary-card__value">{vaultStats.installedLocalCount}</strong>
            <p className="text-muted text-sm">Public catalog and local import</p>
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

      <div className="models-grid">
        <Card className="model-card model-card--wide">
          <div className="model-card__section-header">
            <h3 className="card__title">Model Registry</h3>
            <Button
              variant="primary"
              disabled={!backendConnected}
              onClick={() => setAddModelOpen(true)}
            >
              Add model
            </Button>
          </div>
          {installed.length === 0 ? (
            <p className="text-muted">No local models installed yet.</p>
          ) : (
            installed.map((model) => (
              <div key={model.id} className="model-catalog__row">
                <div>
                  <div className="model-card__header">
                    <div>
                      <h4 className="model-card__name">{model.name}</h4>
                      <p className="text-muted text-sm">
                        {model.provider} · v{model.version}
                        {model.sizeBytes != null
                          ? ` · ${formatBytes(model.sizeBytes)}`
                          : model.sizeGb > 0
                            ? ` · ${model.sizeGb.toFixed(2)} GB`
                            : ""}
                      </p>
                    </div>
                    <StatusBadge status={model.verified ? "installed" : "available"} />
                  </div>
                  <p className="text-muted text-sm">
                    {[
                      model.capabilities.chat && "chat",
                      model.capabilities.completion && "completion",
                      model.capabilities.embeddings && "embeddings",
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </p>
                  <p className="model-card__path mono text-sm">{model.path}</p>
                </div>
                <div className="model-card__actions">
                  <Button
                    variant="ghost"
                    disabled={busyModelId !== null}
                    onClick={() => void handleVerify(model.id)}
                  >
                    Verify
                  </Button>
                  <Button
                    variant="ghost"
                    disabled={busyModelId !== null || model.provider === "remote"}
                    onClick={() => void handleRunInference(model.id)}
                  >
                    Test
                  </Button>
                  <Button
                    variant="ghost"
                    disabled={busyModelId !== null}
                    onClick={() => void handleRemove(model.id)}
                  >
                    Remove
                  </Button>
                </div>
              </div>
            ))
          )}
          {modelTestMessage && (
            <p className="text-muted text-sm judge-test-result">{modelTestMessage}</p>
          )}
        </Card>
      </div>

      <AddModelModal
        open={addModelOpen}
        onClose={() => setAddModelOpen(false)}
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
        onThirdPartySaved={() => void refreshModels()}
      />

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
