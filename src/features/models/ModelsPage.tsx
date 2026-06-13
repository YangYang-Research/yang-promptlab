import { useCallback, useEffect, useState } from "react";

import {
  Button,
  Card,
  PageHeader,
  Select,
  StatusBadge,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  DEFAULT_JUDGE_CONFIG,
  getJudgeConfig,
  JUDGE_MODES,
  LOCAL_PROVIDERS,
  REMOTE_PROVIDERS,
  saveJudgeConfig,
  testJudgeConnectivity,
  testJudgeModel,
  type JudgeConfigDto,
  type JudgeConnectivityResult,
} from "@/shared/ipc/judge";
import {
  browseModels,
  cancelModelDownload,
  getModelDownloadStatus,
  getModelsRegistryInfo,
  getModelsVaultPath,
  importModelGguf,
  importModelZip,
  installModel,
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
} from "@/shared/ipc/models";

export function ModelsPage() {
  const [backendConnected, setBackendConnected] = useState(false);
  const [config, setConfig] = useState<JudgeConfigDto>(DEFAULT_JUDGE_CONFIG);
  const [installed, setInstalled] = useState<ModelEntryDto[]>([]);
  const [catalog, setCatalog] = useState<ModelCatalogEntryDto[]>([]);
  const [registryInfo, setRegistryInfo] = useState<ModelRegistryInfoDto | null>(null);
  const [vaultPath, setVaultPath] = useState<string>("");
  const [importName, setImportName] = useState("");
  const [importPath, setImportPath] = useState("");
  const [importBusy, setImportBusy] = useState<"gguf" | "zip" | null>(null);
  const [downloadProgress, setDownloadProgress] =
    useState<ModelDownloadProgressDto | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState<"connectivity" | "model" | null>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<JudgeConnectivityResult | null>(null);
  const [modelTestMessage, setModelTestMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const refreshModels = useCallback(async () => {
    const [models, entries, info] = await Promise.all([
      listModels(),
      browseModels(),
      getModelsRegistryInfo(),
    ]);
    setInstalled(models);
    setCatalog(entries);
    setRegistryInfo(info);
  }, []);

  const pollDownloadStatus = useCallback(async () => {
    const status = await getModelDownloadStatus();
    if (status.installed) {
      setDownloadProgress(null);
      setDownloadingId(null);
      await refreshModels();
      return;
    }
    setDownloadProgress(status.progress);
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
      setLoading(false);
      return;
    }
    void Promise.all([
      getJudgeConfig(),
      refreshModels(),
      getModelsVaultPath(),
    ])
      .then(([loaded, , path]) => {
        setConfig(loaded);
        setVaultPath(path);
      })
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected, refreshModels]);

  useEffect(() => {
    if (!backendConnected || !downloadProgress) {
      return;
    }
    const timer = window.setInterval(() => {
      void pollDownloadStatus().catch((err) => setError(toAppError(err).message));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [backendConnected, downloadProgress, pollDownloadStatus]);

  function patch(patchValue: Partial<JudgeConfigDto>) {
    setSaved(false);
    setConfig((current) => ({ ...current, ...patchValue }));
  }

  async function handleSave() {
    setError(null);
    setSaving(true);
    try {
      const savedConfig = await saveJudgeConfig(config);
      setConfig(savedConfig);
      setSaved(true);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setSaving(false);
    }
  }

  async function handleTestConnectivity() {
    setError(null);
    setTesting("connectivity");
    setTestResult(null);
    try {
      const result = await testJudgeConnectivity(config);
      setTestResult(result);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setTesting(null);
    }
  }

  async function handleTestModel() {
    setError(null);
    setTesting("model");
    setTestResult(null);
    try {
      const result = await testJudgeModel(config);
      setTestResult(result);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setTesting(null);
    }
  }

  async function handleInstall(entry: ModelCatalogEntryDto) {
    setError(null);
    setInstallingId(entry.id);
    try {
      if (entry.provider === "huggingface") {
        const progress = await startModelDownload({ catalogId: entry.id });
        setDownloadingId(entry.id);
        setDownloadProgress(progress);
      } else {
        await installModel({
          catalogId: entry.id,
          ollamaBaseUrl:
            entry.provider === "ollama" ? config.localBaseUrl : null,
        });
        await refreshModels();
      }
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setInstallingId(null);
    }
  }

  async function handleImport(kind: "gguf" | "zip") {
    if (!importName.trim() || !importPath.trim()) {
      setError("Import name and file path are required");
      return;
    }
    setError(null);
    setImportBusy(kind);
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
      if (config.localVaultModelId === modelId) {
        patch({ localVaultModelId: null, localModelPath: null });
      }
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

  function selectVaultModel(modelId: string) {
    const entry = installed.find((m) => m.id === modelId);
    if (!entry) return;
    if (entry.provider === "ollama") {
      patch({
        localVaultModelId: modelId,
        localProvider: "ollama",
        localModel: entry.version,
      });
    } else {
      patch({
        localVaultModelId: modelId,
        localProvider: "llama_cpp",
        localModelPath: entry.path,
        localModel: entry.name,
      });
    }
  }

  const showLocal = config.mode === "local_llm" || config.mode === "consensus";
  const showRemote = config.mode === "remote_llm";
  const ggufModels = installed.filter(
    (m) => m.provider === "gguf" || m.provider === "huggingface",
  );
  const ollamaModels = installed.filter((m) => m.provider === "ollama");

  return (
    <div className="page">
      <PageHeader
        title="Models"
        description="Install local models (Ollama, HuggingFace, GGUF) and configure the hybrid judge engine"
        actions={
          <Button variant="primary" disabled={saving || loading} onClick={() => void handleSave()}>
            {saving ? "Saving…" : saved ? "Saved" : "Save Judge Config"}
          </Button>
        }
      />

      {!backendConnected && (
        <p className="text-muted">
          Connect to the Tauri backend to manage models and judge configuration.
        </p>
      )}

      {vaultPath && (
        <p className="text-muted text-sm model-vault-path">
          Model vault: <span className="mono">{vaultPath}</span>
        </p>
      )}

      {registryInfo && (
        <p className="text-muted text-sm">
          Registry: {registryInfo.entryCount} entries
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
        <Card className="model-card model-card--wide">
          <h3 className="card__title">Import Local Model</h3>
          <p className="text-muted text-sm">
            Register a local GGUF file or extract one from a ZIP package into the vault.
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
              <label htmlFor="importPath">File path</label>
              <input
                id="importPath"
                className="input mono"
                value={importPath}
                onChange={(e) => setImportPath(e.target.value)}
                placeholder="/path/to/model.gguf or package.zip"
                disabled={!backendConnected || importBusy !== null}
              />
            </div>
          </div>
          <div className="model-card__actions">
            <Button
              variant="secondary"
              disabled={!backendConnected || importBusy !== null}
              onClick={() => void handleImport("gguf")}
            >
              {importBusy === "gguf" ? "Importing…" : "Import GGUF"}
            </Button>
            <Button
              variant="ghost"
              disabled={!backendConnected || importBusy !== null}
              onClick={() => void handleImport("zip")}
            >
              {importBusy === "zip" ? "Importing…" : "Import ZIP"}
            </Button>
          </div>
        </Card>

        <Card className="model-card model-card--wide">
          <h3 className="card__title">Browse Registry</h3>
          <p className="text-muted text-sm">
            Built-in catalog from <code>resources/models.json</code>. Ollama entries call{" "}
            <code>ollama pull</code>; HuggingFace entries download into the vault with
            pause/resume/cancel.
          </p>
          {downloadProgress && (
            <div className="model-download-status">
              <p className="text-sm">
                Downloading {downloadProgress.catalogId}
                {downloadProgress.percent != null
                  ? ` · ${downloadProgress.percent.toFixed(1)}%`
                  : ` · ${(downloadProgress.downloadedBytes / (1024 * 1024)).toFixed(1)} MB`}
                {" · "}
                {downloadProgress.status}
              </p>
              <div className="model-card__actions">
                <Button
                  variant="ghost"
                  disabled={!backendConnected}
                  onClick={() =>
                    void pauseModelDownload()
                      .then(setDownloadProgress)
                      .catch((err) => setError(toAppError(err).message))
                  }
                >
                  Pause
                </Button>
                <Button
                  variant="ghost"
                  disabled={!backendConnected}
                  onClick={() =>
                    void resumeModelDownload()
                      .then(setDownloadProgress)
                      .catch((err) => setError(toAppError(err).message))
                  }
                >
                  Resume
                </Button>
                <Button
                  variant="ghost"
                  disabled={!backendConnected}
                  onClick={() =>
                    void cancelModelDownload()
                      .then(() => {
                        setDownloadProgress(null);
                        setDownloadingId(null);
                      })
                      .catch((err) => setError(toAppError(err).message))
                  }
                >
                  Cancel
                </Button>
              </div>
            </div>
          )}
          <div className="model-catalog">
            {catalog.map((entry) => (
              <div key={entry.id} className="model-catalog__row">
                <div>
                  <strong>
                    {entry.name}
                    {entry.recommended ? " · recommended" : ""}
                  </strong>
                  <p className="text-muted text-sm">{entry.description}</p>
                  <p className="text-muted text-sm">
                    {entry.provider} · {entry.purpose}
                    {entry.quant ? ` · ${entry.quant}` : ""}
                    {entry.sizeGb != null ? ` · ${entry.sizeGb.toFixed(1)} GB` : ""}
                  </p>
                </div>
                <Button
                  variant="secondary"
                  disabled={
                    !backendConnected ||
                    installingId !== null ||
                    (downloadingId !== null && downloadingId !== entry.id)
                  }
                  onClick={() => void handleInstall(entry)}
                >
                  {downloadingId === entry.id
                    ? "Downloading…"
                    : installingId === entry.id
                      ? "Installing…"
                      : entry.provider === "huggingface"
                        ? "Download"
                        : "Install"}
                </Button>
              </div>
            ))}
          </div>
        </Card>

        <Card className="model-card model-card--wide">
          <h3 className="card__title">Installed Models</h3>
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
                        {model.sizeGb > 0 ? ` · ${model.sizeGb.toFixed(2)} GB` : ""}
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
                    disabled={busyModelId !== null}
                    onClick={() => void handleRunInference(model.id)}
                  >
                    Test
                  </Button>
                  <Button
                    variant="ghost"
                    disabled={busyModelId !== null}
                    onClick={() => selectVaultModel(model.id)}
                  >
                    Use for Judge
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
          <h3 className="card__title">Judge Provider</h3>

          <div className="settings-field">
            <label htmlFor="judgeMode">Judge Mode</label>
            <Select
              id="judgeMode"
              value={config.mode}
              onChange={(e) => patch({ mode: e.target.value as JudgeConfigDto["mode"] })}
              disabled={loading}
            >
              {JUDGE_MODES.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </Select>
            <p className="text-muted text-sm">
              {JUDGE_MODES.find((option) => option.value === config.mode)?.hint}
            </p>
          </div>

          {showLocal && (
            <div className="wizard-auth-fields">
              <div className="settings-field">
                <label htmlFor="vaultModel">Vault Model (optional)</label>
                <Select
                  id="vaultModel"
                  value={config.localVaultModelId ?? ""}
                  onChange={(e) => {
                    const id = e.target.value;
                    if (!id) {
                      patch({ localVaultModelId: null });
                      return;
                    }
                    selectVaultModel(id);
                  }}
                >
                  <option value="">Manual configuration</option>
                  <optgroup label="GGUF / HuggingFace">
                    {ggufModels.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.name}
                      </option>
                    ))}
                  </optgroup>
                  <optgroup label="Ollama">
                    {ollamaModels.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.name}
                      </option>
                    ))}
                  </optgroup>
                </Select>
              </div>

              <div className="settings-field">
                <label htmlFor="localProvider">Local Provider</label>
                <Select
                  id="localProvider"
                  value={config.localProvider}
                  onChange={(e) =>
                    patch({ localProvider: e.target.value as JudgeConfigDto["localProvider"] })
                  }
                >
                  {LOCAL_PROVIDERS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </Select>
              </div>

              {config.localProvider === "ollama" ? (
                <>
                  <div className="settings-field">
                    <label htmlFor="localBaseUrl">Ollama Base URL</label>
                    <input
                      id="localBaseUrl"
                      className="input mono"
                      value={config.localBaseUrl}
                      onChange={(e) => patch({ localBaseUrl: e.target.value })}
                    />
                  </div>
                  <div className="settings-field">
                    <label htmlFor="localModel">Model</label>
                    <input
                      id="localModel"
                      className="input"
                      placeholder="llama3, qwen2.5, deepseek-r1, gemma2"
                      value={config.localModel}
                      onChange={(e) => patch({ localModel: e.target.value })}
                    />
                  </div>
                </>
              ) : (
                <>
                  <div className="settings-field">
                    <label htmlFor="localModelPath">GGUF Model Path</label>
                    <input
                      id="localModelPath"
                      className="input mono"
                      value={config.localModelPath ?? ""}
                      onChange={(e) =>
                        patch({ localModelPath: e.target.value.trim() || null })
                      }
                    />
                  </div>
                  <div className="settings-field">
                    <label htmlFor="localLlamaBinary">llama-server Binary</label>
                    <input
                      id="localLlamaBinary"
                      className="input mono"
                      value={config.localLlamaBinary}
                      onChange={(e) => patch({ localLlamaBinary: e.target.value })}
                    />
                  </div>
                </>
              )}
            </div>
          )}

          {showRemote && (
            <div className="wizard-auth-fields">
              <div className="settings-field">
                <label htmlFor="remoteProvider">Remote Provider</label>
                <Select
                  id="remoteProvider"
                  value={config.remoteProvider}
                  onChange={(e) =>
                    patch({ remoteProvider: e.target.value as JudgeConfigDto["remoteProvider"] })
                  }
                >
                  {REMOTE_PROVIDERS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </Select>
              </div>
              <div className="settings-field">
                <label htmlFor="remoteModel">Model</label>
                <input
                  id="remoteModel"
                  className="input"
                  value={config.remoteModel}
                  onChange={(e) => patch({ remoteModel: e.target.value })}
                />
              </div>
              <div className="settings-field">
                <label htmlFor="remoteBaseUrl">Custom Base URL (optional)</label>
                <input
                  id="remoteBaseUrl"
                  className="input mono"
                  value={config.remoteBaseUrl ?? ""}
                  onChange={(e) =>
                    patch({ remoteBaseUrl: e.target.value.trim() || null })
                  }
                />
              </div>
              <div className="settings-field">
                <label htmlFor="remoteApiKey">API Key</label>
                <input
                  id="remoteApiKey"
                  className="input mono"
                  type="password"
                  value={config.remoteApiKey}
                  onChange={(e) => patch({ remoteApiKey: e.target.value })}
                />
              </div>
              <div className="settings-field">
                <label htmlFor="remoteApiKeyEnv">API Key Env Var (fallback)</label>
                <input
                  id="remoteApiKeyEnv"
                  className="input mono"
                  placeholder="OPENAI_API_KEY"
                  value={config.remoteApiKeyEnv ?? ""}
                  onChange={(e) =>
                    patch({ remoteApiKeyEnv: e.target.value.trim() || null })
                  }
                />
              </div>
            </div>
          )}

          <div className="model-card__actions">
            <Button
              variant="secondary"
              disabled={!backendConnected || testing !== null}
              onClick={() => void handleTestConnectivity()}
            >
              {testing === "connectivity" ? "Testing…" : "Validate Connectivity"}
            </Button>
            <Button
              variant="ghost"
              disabled={!backendConnected || testing !== null || config.mode === "deterministic"}
              onClick={() => void handleTestModel()}
            >
              {testing === "model" ? "Running…" : "Test Model"}
            </Button>
          </div>

          {testResult && (
            <div className="judge-test-result">
              <StatusBadge status={testResult.ok ? "completed" : "failed"} />
              <p className="text-sm">
                {testResult.provider} / {testResult.model} — {testResult.message}
                {testResult.latencyMs > 0 ? ` (${testResult.latencyMs} ms)` : ""}
              </p>
              {testResult.sampleResponse && (
                <p className="text-muted text-sm">{testResult.sampleResponse}</p>
              )}
            </div>
          )}
        </Card>
      </div>

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
