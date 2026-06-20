import { useEffect, useState } from "react";

import { Button, Card, PageHeader, Select, StatusBadge } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  DEFAULT_JUDGE_CONFIG,
  getJudgeConfig,
  JUDGE_MODES,
  LOCAL_PROVIDERS,
  saveJudgeConfig,
  testJudgeConnectivity,
  testJudgeModel,
  type JudgeConfigDto,
  type JudgeConnectivityResult,
} from "@/shared/ipc/judge";
import { listModels, type ModelEntryDto } from "@/shared/ipc/models";
import { getRuntimeStatus, type RuntimeStatusDto } from "@/shared/ipc/runtime";

export function JudgeProviderPage() {
  const [backendConnected, setBackendConnected] = useState(false);
  const [config, setConfig] = useState<JudgeConfigDto>(DEFAULT_JUDGE_CONFIG);
  const [installed, setInstalled] = useState<ModelEntryDto[]>([]);
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatusDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState<"connectivity" | "model" | null>(null);
  const [testResult, setTestResult] = useState<JudgeConnectivityResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

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
    void Promise.all([getJudgeConfig(), listModels(), getRuntimeStatus()])
      .then(([loaded, models, runtime]) => {
        setConfig(loaded);
        setInstalled(models);
        setRuntimeStatus(runtime);
      })
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected]);

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
      setTestResult(await testJudgeConnectivity(config));
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
      setTestResult(await testJudgeModel(config));
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setTesting(null);
    }
  }

  function selectVaultModel(modelId: string) {
    const entry = installed.find((m) => m.id === modelId);
    if (!entry) return;
    patch({
      localVaultModelId: modelId,
      localProvider: "llama_cpp",
      localModelPath: entry.path,
      localModel: entry.name,
    });
  }

  const showLocal = config.mode === "local_llm" || config.mode === "consensus";
  const ggufModels = installed.filter(
    (m) => m.provider === "gguf" || m.provider === "huggingface" || m.format === "gguf",
  );

  return (
    <div className="page">
      <PageHeader
        title="Judge Provider"
        description="Configure the hybrid judge engine for vulnerability verdicts"
        actions={
          <Button variant="primary" disabled={saving || loading} onClick={() => void handleSave()}>
            {saving ? "Saving…" : saved ? "Saved" : "Save Judge Config"}
          </Button>
        }
      />

      {!backendConnected && (
        <p className="text-muted">Connect to the Tauri backend to configure the judge provider.</p>
      )}

      {backendConnected && (
        <div className="models-summary-grid judge-runtime-grid">
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

      <Card className="model-card model-card--wide">
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
                {ggufModels.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
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
                {LOCAL_PROVIDERS.filter((option) => option.value === "llama_cpp").map(
                  (option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ),
                )}
              </Select>
            </div>
            <div className="settings-field">
              <label htmlFor="localModelPath">GGUF Model Path</label>
              <input
                id="localModelPath"
                className="input mono"
                value={config.localModelPath ?? ""}
                onChange={(e) => patch({ localModelPath: e.target.value.trim() || null })}
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
          </div>
        )}

        {config.mode === "remote_llm" && (
          <p className="text-muted text-sm">
            Configure OpenAI, Anthropic, Google, Azure, or AWS Bedrock under{" "}
            <strong>Models → Third-party Models</strong>.
          </p>
        )}

        {config.mode !== "remote_llm" && (
          <>
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
          </>
        )}
      </Card>

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
