import { useEffect, useState } from "react";

import { Button } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  DEFAULT_JUDGE_CONFIG,
  getJudgeConfig,
  saveJudgeConfig,
  testJudgeConnectivity,
  THIRD_PARTY_PROVIDERS,
  thirdPartyTestConfig,
  validateThirdPartyConfig,
  type JudgeConfigDto,
  type JudgeConnectivityResult,
  type ThirdPartyProvider,
} from "@/shared/ipc/judge";
import { saveThirdPartyModel } from "@/shared/ipc/models";
import { useToast } from "@/shared/notifications";

type ThirdPartyModelsPanelProps = {
  backendConnected: boolean;
  onSaved?: () => void;
};

function providerIcon(label: string): string {
  return label.trim().charAt(0).toUpperCase();
}

function isThirdPartyProvider(value: string): value is ThirdPartyProvider {
  return THIRD_PARTY_PROVIDERS.some((provider) => provider.value === value);
}

function connectivityToast(
  result: JudgeConnectivityResult,
): { message: string; type: "success" | "error" } {
  const latency = result.latencyMs > 0 ? ` (${result.latencyMs} ms)` : "";
  const label = `${result.provider} / ${result.model}`;
  if (result.ok) {
    return {
      type: "success",
      message: `Connection successful — ${label}${latency}`,
    };
  }
  return {
    type: "error",
    message: `Connection failed — ${label}: ${result.message}`,
  };
}

export function ThirdPartyModelsPanel({
  backendConnected,
  onSaved,
}: ThirdPartyModelsPanelProps) {
  const { notify } = useToast();
  const [config, setConfig] = useState<JudgeConfigDto>(DEFAULT_JUDGE_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const selectedProvider =
    THIRD_PARTY_PROVIDERS.find((provider) => provider.value === config.remoteProvider) ??
    THIRD_PARTY_PROVIDERS[0];
  const isBedrock = config.remoteProvider === "bedrock";
  const accessKeyRequiresSessionToken = config.remoteApiKey.trim().startsWith("ASIA");

  useEffect(() => {
    if (!backendConnected) {
      setLoading(false);
      return;
    }
    void getJudgeConfig()
      .then((loaded) => {
        const provider = isThirdPartyProvider(loaded.remoteProvider)
          ? loaded.remoteProvider
          : "openai";
        const next = { ...loaded, remoteProvider: provider };
        // Clear stale OpenAI default model when viewing Bedrock without a saved model.
        if (provider === "bedrock" && next.remoteModel.trim() === "gpt-4o-mini") {
          next.remoteModel = "";
        }
        setConfig(next);
      })
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected]);

  function patchRemote(patchValue: Partial<JudgeConfigDto>) {
    setSaved(false);
    setConfig((current) => ({ ...current, ...patchValue }));
  }

  function selectProvider(provider: ThirdPartyProvider) {
    const meta = THIRD_PARTY_PROVIDERS.find((entry) => entry.value === provider);
    if (!meta) return;
    patchRemote({
      remoteProvider: provider,
      remoteModel: "",
      remoteApiKey: "",
      remoteAwsSecretAccessKey: "",
      remoteApiKeyEnv: meta.apiKeyEnv,
      remoteAwsRegion: provider === "bedrock" ? "" : null,
      remoteAwsSessionToken: "",
      remoteBaseUrl: null,
      remoteApiKeyConfigured: false,
      remoteAwsSecretAccessKeyConfigured: false,
      remoteAwsSessionTokenConfigured: false,
    });
  }

  async function handleSave() {
    const validationError = validateThirdPartyConfig(config);
    if (validationError) {
      notify(validationError, "error");
      return;
    }

    setError(null);
    setSaving(true);
    try {
      const savedConfig = await saveJudgeConfig(config);
      await saveThirdPartyModel({
        provider: config.remoteProvider,
        model: config.remoteModel.trim(),
        baseUrl: config.remoteBaseUrl,
        region: config.remoteAwsRegion,
      });
      setConfig(savedConfig);
      setSaved(true);
      notify("Third-party model saved", "success");
      onSaved?.();
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setSaving(false);
    }
  }

  async function handleTest() {
    const validationError = validateThirdPartyConfig(config);
    if (validationError) {
      notify(validationError, "error");
      return;
    }

    setError(null);
    setTesting(true);
    try {
      const result = await testJudgeConnectivity(thirdPartyTestConfig(config));
      const toast = connectivityToast(result);
      notify(toast.message, toast.type);
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="third-party-models">
      <p className="text-muted text-sm">
        Configure cloud LLM providers for remote inference and judge mode. Credentials are stored
        securely in the OS keychain when available.
      </p>

      <div className="third-party-models__grid">
        {THIRD_PARTY_PROVIDERS.map((provider) => {
          const isSelected = config.remoteProvider === provider.value;
          return (
            <button
              key={provider.value}
              type="button"
              className={`third-party-models__card ${isSelected ? "third-party-models__card--selected" : ""}`}
              disabled={!backendConnected || loading}
              onClick={() => selectProvider(provider.value)}
            >
              <span className="third-party-models__icon" aria-hidden="true">
                {providerIcon(provider.label)}
              </span>
              <span className="third-party-models__name">{provider.label}</span>
            </button>
          );
        })}
      </div>

      <div className="third-party-models__form">
        {isBedrock ? (
          <>
            <div className="settings-field">
              <label htmlFor="bedrockAccessKeyId">Access Key ID</label>
              <input
                id="bedrockAccessKeyId"
                className="input mono"
                placeholder="AKIA... or ASIA..."
                value={config.remoteApiKey}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteApiKey: e.target.value })}
                autoComplete="off"
              />
            </div>

            <div className="settings-field">
              <label htmlFor="bedrockSecretAccessKey">Secret Access Key</label>
              <input
                id="bedrockSecretAccessKey"
                className="input mono"
                type="password"
                placeholder={
                  config.remoteAwsSecretAccessKeyConfigured ? "Configured in keychain" : "••••••••"
                }
                value={config.remoteAwsSecretAccessKey}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteAwsSecretAccessKey: e.target.value })}
                autoComplete="off"
              />
            </div>

            {(accessKeyRequiresSessionToken || config.remoteAwsSessionTokenConfigured) && (
              <div className="settings-field">
                <label htmlFor="bedrockSessionToken">Session Token</label>
                <input
                  id="bedrockSessionToken"
                  className="input mono"
                  type="password"
                  placeholder={
                    config.remoteAwsSessionTokenConfigured
                      ? "Configured in keychain"
                      : "Required for ASIA (temporary) credentials"
                  }
                  value={config.remoteAwsSessionToken}
                  disabled={!backendConnected || loading}
                  onChange={(e) => patchRemote({ remoteAwsSessionToken: e.target.value })}
                  autoComplete="off"
                />
              </div>
            )}

            <div className="settings-field">
              <label htmlFor="bedrockRegion">Region</label>
              <input
                id="bedrockRegion"
                className="input mono"
                placeholder={selectedProvider.regionPlaceholder ?? "AWS region"}
                value={config.remoteAwsRegion ?? ""}
                disabled={!backendConnected || loading}
                onChange={(e) =>
                  patchRemote({ remoteAwsRegion: e.target.value.trim() || null })
                }
              />
            </div>

            <div className="settings-field">
              <label htmlFor="bedrockModel">Model</label>
              <input
                id="bedrockModel"
                className="input mono"
                placeholder={selectedProvider.modelPlaceholder}
                value={config.remoteModel}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteModel: e.target.value })}
              />
            </div>
          </>
        ) : (
          <>
            <div className="settings-field">
              <label htmlFor="thirdPartyModel">Model</label>
              <input
                id="thirdPartyModel"
                className="input"
                placeholder={selectedProvider.modelPlaceholder}
                value={config.remoteModel}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteModel: e.target.value })}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyBaseUrl">
                {selectedProvider.requiresBaseUrl ? "Endpoint URL" : "Custom Base URL (optional)"}
              </label>
              <input
                id="thirdPartyBaseUrl"
                className="input mono"
                placeholder={
                  selectedProvider.baseUrlPlaceholder ?? "https://api.openai.com/v1"
                }
                value={config.remoteBaseUrl ?? ""}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteBaseUrl: e.target.value.trim() || null })}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyApiKey">API Key</label>
              <input
                id="thirdPartyApiKey"
                className="input mono"
                type="password"
                placeholder={config.remoteApiKeyConfigured ? "Configured in keychain" : "sk-..."}
                value={config.remoteApiKey}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteApiKey: e.target.value })}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyApiKeyEnv">API Key Env Var (fallback)</label>
              <input
                id="thirdPartyApiKeyEnv"
                className="input mono"
                placeholder={selectedProvider.apiKeyEnv}
                value={config.remoteApiKeyEnv ?? ""}
                disabled={!backendConnected || loading}
                onChange={(e) => patchRemote({ remoteApiKeyEnv: e.target.value.trim() || null })}
              />
            </div>
          </>
        )}
      </div>

      <div className="model-card__actions">
        <Button
          variant="secondary"
          disabled={!backendConnected || testing || loading}
          onClick={() => void handleTest()}
        >
          {testing ? "Testing…" : "Test Connection"}
        </Button>
        <Button
          variant="primary"
          disabled={!backendConnected || saving || loading}
          onClick={() => void handleSave()}
        >
          {saving ? "Saving…" : saved ? "Saved" : "Save config"}
        </Button>
      </div>

      {error && <p className="text-danger text-sm">{error}</p>}
    </div>
  );
}
