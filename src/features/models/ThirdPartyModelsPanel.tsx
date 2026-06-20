import { useState } from "react";

import { Button } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  saveThirdPartyModelForm,
  testThirdPartyModelConnectivity,
  THIRD_PARTY_PROVIDERS,
  thirdPartyModelTemplate,
  validateThirdPartyModelForm,
  type ThirdPartyModelConnectivityResult,
  type ThirdPartyModelForm,
  type ThirdPartyProvider,
} from "@/shared/ipc/thirdPartyModels";
import { useToast } from "@/shared/notifications";

type ThirdPartyModelsPanelProps = {
  backendConnected: boolean;
  onSaved?: () => void;
};

function providerIcon(label: string): string {
  return label.trim().charAt(0).toUpperCase();
}

function connectivityToast(
  result: ThirdPartyModelConnectivityResult,
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
  const [form, setForm] = useState<ThirdPartyModelForm>(() =>
    thirdPartyModelTemplate("openai"),
  );
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const selectedProvider =
    THIRD_PARTY_PROVIDERS.find((provider) => provider.value === form.provider) ??
    THIRD_PARTY_PROVIDERS[0];
  const isBedrock = form.provider === "bedrock";
  const accessKeyRequiresSessionToken = form.apiKey.trim().startsWith("ASIA");

  function patchForm(patchValue: Partial<ThirdPartyModelForm>) {
    setSaved(false);
    setForm((current) => ({ ...current, ...patchValue }));
  }

  function selectProvider(provider: ThirdPartyProvider) {
    setForm(thirdPartyModelTemplate(provider));
    setSaved(false);
  }

  async function handleSave() {
    const validationError = validateThirdPartyModelForm(form);
    if (validationError) {
      notify(validationError, "error");
      return;
    }

    setError(null);
    setSaving(true);
    try {
      const savedForm = await saveThirdPartyModelForm(form);
      setForm(savedForm);
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
    const validationError = validateThirdPartyModelForm(form);
    if (validationError) {
      notify(validationError, "error");
      return;
    }

    setError(null);
    setTesting(true);
    try {
      const result = await testThirdPartyModelConnectivity(form);
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
        Register cloud LLM providers for remote inference. Credentials are stored securely in the
        OS keychain when available.
      </p>

      <div className="third-party-models__grid">
        {THIRD_PARTY_PROVIDERS.map((provider) => {
          const isSelected = form.provider === provider.value;
          return (
            <button
              key={provider.value}
              type="button"
              className={`third-party-models__card ${isSelected ? "third-party-models__card--selected" : ""}`}
              disabled={!backendConnected}
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
                value={form.apiKey}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ apiKey: e.target.value })}
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
                  form.awsSecretAccessKeyConfigured ? "Configured in keychain" : "••••••••"
                }
                value={form.awsSecretAccessKey}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ awsSecretAccessKey: e.target.value })}
                autoComplete="off"
              />
            </div>

            {(accessKeyRequiresSessionToken || form.awsSessionTokenConfigured) && (
              <div className="settings-field">
                <label htmlFor="bedrockSessionToken">Session Token</label>
                <input
                  id="bedrockSessionToken"
                  className="input mono"
                  type="password"
                  placeholder={
                    form.awsSessionTokenConfigured
                      ? "Configured in keychain"
                      : "Required for ASIA (temporary) credentials"
                  }
                  value={form.awsSessionToken}
                  disabled={!backendConnected}
                  onChange={(e) => patchForm({ awsSessionToken: e.target.value })}
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
                value={form.awsRegion ?? ""}
                disabled={!backendConnected}
                onChange={(e) =>
                  patchForm({ awsRegion: e.target.value.trim() || null })
                }
              />
            </div>

            <div className="settings-field">
              <label htmlFor="bedrockModel">Model</label>
              <input
                id="bedrockModel"
                className="input mono"
                placeholder={selectedProvider.modelPlaceholder}
                value={form.model}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ model: e.target.value })}
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
                value={form.model}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ model: e.target.value })}
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
                value={form.baseUrl ?? ""}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ baseUrl: e.target.value.trim() || null })}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyApiKey">API Key</label>
              <input
                id="thirdPartyApiKey"
                className="input mono"
                type="password"
                placeholder={form.apiKeyConfigured ? "Configured in keychain" : "sk-..."}
                value={form.apiKey}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ apiKey: e.target.value })}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyApiKeyEnv">API Key Env Var (fallback)</label>
              <input
                id="thirdPartyApiKeyEnv"
                className="input mono"
                placeholder={selectedProvider.apiKeyEnv}
                value={form.apiKeyEnv ?? ""}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ apiKeyEnv: e.target.value.trim() || null })}
              />
            </div>
          </>
        )}
      </div>

      <div className="model-card__actions">
        <Button
          variant="secondary"
          disabled={!backendConnected || testing}
          onClick={() => void handleTest()}
        >
          {testing ? "Testing…" : "Test Connection"}
        </Button>
        <Button
          variant="primary"
          disabled={!backendConnected || saving}
          onClick={() => void handleSave()}
        >
          {saving ? "Saving…" : saved ? "Saved" : "Save config"}
        </Button>
      </div>

      {error && <p className="text-danger text-sm">{error}</p>}
    </div>
  );
}
