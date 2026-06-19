import { useCallback, useEffect, useMemo, useState } from "react";

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
import { getRuntimeStatus, type RuntimeStatusDto } from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";
import { formatBytes } from "@/shared/utils/format";
import { DownloadManagerCard } from "./DownloadManagerCard";
import { HuggingFaceModelCatalog } from "./HuggingFaceModelCatalog";
import { ThirdPartyModelsPanel } from "./ThirdPartyModelsPanel";

export function ModelsPage() {
  const { notify } = useToast();
  const [backendConnected, setBackendConnected] = useState(false);
  const [installed, setInstalled] = useState<ModelEntryDto[]>([]);
  const [catalog, setCatalog] = useState<ModelCatalogEntryDto[]>([]);
  const [registryInfo, setRegistryInfo] = useState<ModelRegistryInfoDto | null>(null);
  const [vaultPath, setVaultPath] = useState<string>("");
  const [vaultStats, setVaultStats] = useState<ModelVaultStatsDto | null>(null);
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatusDto | null>(null);
  const [importName, setImportName] = useState("");
  const [importPath, setImportPath] = useState("");
  const [importBusy, setImportBusy] = useState<"browse" | "import" | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<ModelDownloadProgressDto | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [modelTestMessage, setModelTestMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const installedNames = useMemo(() => new Set(installed.map((m) => m.name)), [installed]);

  const refreshModels = useCallback(async () => {
    const [models, entries, info, stats, runtime] = await Promise.all([
      listModels(),
      browseModels(),
      getModelsRegistryInfo(),
      getModelsVaultStats(),
      getRuntimeStatus(),
    ]);
    setInstalled(models);
    setCatalog(entries);
    setRegistryInfo(info);
    setVaultStats(stats);
    setRuntimeStatus(runtime);
  }, []);

  const pollDownloadStatus = useCallback(async () => {
    const status = await getModelDownloadStatus();
    if (status.installed) {
      setDownloadProgress(null);
      setDownloadingId(null);
      await refreshModels();
      return;
    }
    if (status.progress) {
      setDownloadProgress(status.progress);
      if (status.progress.status === "failed") {
        setDownloadingId(null);
        setError(status.progress.error ?? "Model download failed");
      } else {
        setDownloadingId(status.progress.catalogId);
      }
    } else {
      setDownloadProgress(null);
      setDownloadingId(null);
    }
  }, [refreshModels]);

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
    void Promise.all([refreshModels(), getModelsVaultPath(), pollDownloadStatus()])
      .then(([, path]) => {
        setVaultPath(path);
      })
      .catch((err) => setError(toAppError(err).message));
  }, [backendConnected, refreshModels, pollDownloadStatus]);

  useEffect(() => {
    if (!backendConnected) {
      return;
    }
    const timer = window.setInterval(() => {
      void getRuntimeStatus()
        .then(setRuntimeStatus)
        .catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [backendConnected]);

  useEffect(() => {
    if (!backendConnected || !downloadProgress) {
      return;
    }
    // Stop polling once the download reaches a terminal state so the failed/completed
    // card (and its error message) stays visible instead of being cleared.
    if (downloadProgress.status === "failed" || downloadProgress.status === "completed") {
      return;
    }
    const timer = window.setInterval(() => {
      void pollDownloadStatus().catch((err) => setError(toAppError(err).message));
    }, 750);
    return () => window.clearInterval(timer);
  }, [backendConnected, downloadProgress, pollDownloadStatus]);

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
            <span className="models-summary-card__label">Installed models</span>
            <strong className="models-summary-card__value">{vaultStats.modelCount}</strong>
            <p className="text-muted text-sm">
              {formatBytes(vaultStats.installedBytes)} registered
            </p>
          </Card>
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Installed size</span>
            <strong className="models-summary-card__value">
              {vaultStats.installedGb.toFixed(2)} GB
            </strong>
            <p className="text-muted text-sm">{formatBytes(vaultStats.installedBytes)}</p>
          </Card>
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Vault disk usage</span>
            <strong className="models-summary-card__value">
              {vaultStats.diskUsageGb.toFixed(2)} GB
            </strong>
            <p className="text-muted text-sm">{formatBytes(vaultStats.diskUsageBytes)} on disk</p>
          </Card>
          <Card className="models-summary-card">
            <span className="models-summary-card__label">Runtime status</span>
            <strong className="models-summary-card__value models-summary-card__value--runtime">
              {runtimeStatus?.healthy
                ? "Healthy"
                : runtimeStatus?.state === "running"
                  ? "Running"
                  : runtimeStatus?.state === "starting"
                    ? "Starting"
                    : runtimeStatus?.state === "failed"
                      ? "Failed"
                      : "Stopped"}
            </strong>
            <p className="text-muted text-sm">
              {runtimeStatus?.message ?? "Checking embedded llama.cpp runtime…"}
            </p>
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

      <div className="models-grid">
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
          />
        )}

        <Card className="model-card model-card--wide">
          <h3 className="card__title">Manage Models</h3>
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

        <Card className="model-card model-card--wide">
          <h3 className="card__title">Public Models</h3>
          <p className="text-muted text-sm">
            Built-in GGUF catalog from <code>resources/models.json</code>. Click a card for
            details; use <strong>+</strong> to add to your vault.
          </p>
          <HuggingFaceModelCatalog
            catalog={catalog}
            installedNames={installedNames}
            downloadingId={downloadingId}
            installingId={installingId}
            backendConnected={backendConnected}
            onInstall={(entry) => void handleInstall(entry)}
          />
        </Card>

        <Card className="model-card model-card--wide">
          <h3 className="card__title">Third-party Models</h3>
          <ThirdPartyModelsPanel
            backendConnected={backendConnected}
            onSaved={() => void refreshModels()}
          />
        </Card>

        <Card className="model-card model-card--wide">
          <h3 className="card__title">Import Model</h3>
          <p className="text-muted text-sm">
            Use the native file picker to register a GGUF file or extract one from a ZIP package
            into the vault.
          </p>
          <div className="wizard-auth-fields">
            <div className="settings-field">
              <label htmlFor="importName">Display name</label>
              <input
                id="importName"
                className="input"
                value={importName}
                onChange={(e) => setImportName(e.target.value)}
                disabled={!backendConnected || importBusy !== null}
              />
            </div>
            <div className="settings-field">
              <label htmlFor="importPath">Selected file</label>
              <div className="import-path-row">
                <input
                  id="importPath"
                  className="input mono"
                  value={importPath}
                  readOnly
                  placeholder="Browse for a .gguf or .zip file"
                  disabled={!backendConnected || importBusy !== null}
                />
                <Button
                  variant="secondary"
                  disabled={!backendConnected || importBusy !== null}
                  onClick={() => void handleBrowse()}
                >
                  {importBusy === "browse" ? "Opening…" : "Browse"}
                </Button>
              </div>
            </div>
          </div>
          <div className="model-card__actions">
            <Button
              variant="secondary"
              disabled={!backendConnected || importBusy !== null}
              onClick={() => void handleImport()}
            >
              {importBusy === "import" ? "Importing…" : "Import"}
            </Button>
          </div>
        </Card>
      </div>

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
