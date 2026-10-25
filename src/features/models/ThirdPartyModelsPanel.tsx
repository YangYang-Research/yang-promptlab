import { useEffect, useRef, useState } from "react";

import { Button, IconButton, IconExternalLink } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { recordLocalActivity } from "@/shared/activity/localActivity";
import {
  listProviderModels,
  saveThirdPartyModelForm,
  testThirdPartyModelConnectivity,
  thirdPartyConnectionTestFingerprint,
  THIRD_PARTY_PROVIDERS,
  thirdPartyModelTemplate,
  validateThirdPartyModelForm,
  type ThirdPartyModelConnectivityResult,
  type ThirdPartyModelForm,
  type ThirdPartyProvider,
} from "@/shared/ipc/thirdPartyModels";
import { useToast } from "@/shared/notifications";
import { openExternalUrl } from "@/shared/utils/openExternalUrl";

import { ProviderLogo } from "./ProviderLogo";

type ThirdPartyModelsPanelProps = {
  backendConnected: boolean;
  initialForm?: ThirdPartyModelForm | null;
  editingModelId?: string | null;
  onSaved?: () => void;
};

function ModelIdLabel({
  htmlFor,
  providerLabel,
  modelsDocsUrl,
}: {
  htmlFor: string;
  providerLabel: string;
  modelsDocsUrl?: string;
}) {
  return (
    <div className="settings-field__label-row">
      <label htmlFor={htmlFor}>Model ID</label>
      {modelsDocsUrl ? (
        <IconButton
          ariaLabel={`Open ${providerLabel} model catalog`}
          size="sm"
          onClick={() => void openExternalUrl(modelsDocsUrl)}
        >
          <IconExternalLink />
        </IconButton>
      ) : null}
    </div>
  );
}

function catalogFingerprint(form: ThirdPartyModelForm): string {
  return JSON.stringify({
    provider: form.provider,
    customProviderName: form.customProviderName.trim(),
    baseUrl: form.baseUrl?.trim() || "",
    apiKey: form.apiKey.trim() || (form.apiKeyConfigured ? "configured" : ""),
    apiKeyEnv: form.apiKeyEnv?.trim() || "",
    awsSecretAccessKey:
      form.awsSecretAccessKey.trim() || (form.awsSecretAccessKeyConfigured ? "configured" : ""),
    awsSessionToken:
      form.awsSessionToken.trim() || (form.awsSessionTokenConfigured ? "configured" : ""),
    awsRegion: form.awsRegion?.trim() || "",
  });
}

function catalogReady(form: ThirdPartyModelForm): boolean {
  if (form.provider === "bedrock") {
    const key = form.apiKey.trim() || form.apiKeyConfigured;
    const secret = form.awsSecretAccessKey.trim() || form.awsSecretAccessKeyConfigured;
    return Boolean(key && secret);
  }
  if (form.provider === "custom") {
    return Boolean(form.baseUrl?.trim());
  }
  return Boolean(form.apiKey.trim() || form.apiKeyConfigured || form.apiKeyEnv?.trim());
}

function ModelIdPicker({
  id,
  value,
  placeholder,
  disabled,
  mono,
  loading,
  open,
  options,
  onChange,
  onFocus,
  onBlur,
  onPick,
}: {
  id: string;
  value: string;
  placeholder: string;
  disabled: boolean;
  mono?: boolean;
  loading: boolean;
  open: boolean;
  options: string[];
  onChange: (value: string) => void;
  onFocus: () => void;
  onBlur: () => void;
  onPick: (value: string) => void;
}) {
  const [filterQuery, setFilterQuery] = useState<string | null>(null);
  const query = (filterQuery ?? "").trim().toLowerCase();
  const shown = options
    .filter((modelId) => !query || modelId.toLowerCase().includes(query))
    .slice(0, 200);
  const showList = open && (loading || shown.length > 0);

  return (
    <div className="third-party-models__picker">
      <input
        id={id}
        className={mono ? "input mono" : "input"}
        placeholder={placeholder}
        value={value}
        disabled={disabled}
        role="combobox"
        aria-expanded={showList}
        aria-controls={`${id}-catalog`}
        aria-autocomplete="list"
        onChange={(event) => {
          setFilterQuery(event.target.value);
          onChange(event.target.value);
        }}
        onFocus={() => {
          setFilterQuery(null);
          onFocus();
        }}
        onBlur={onBlur}
      />
      {showList ? (
        <ul id={`${id}-catalog`} className="third-party-models__catalog" role="listbox">
          {loading && shown.length === 0 ? (
            <li className="third-party-models__catalog-status">Loading models…</li>
          ) : (
            shown.map((modelId) => (
              <li key={modelId}>
                <button
                  type="button"
                  role="option"
                  aria-selected={modelId === value}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => onPick(modelId)}
                >
                  {modelId}
                </button>
              </li>
            ))
          )}
        </ul>
      ) : null}
    </div>
  );
}

function connectivityToast(
  result: ThirdPartyModelConnectivityResult,
): { message: string; type: "success" | "error" } {
  const latency = result.latencyMs > 0 ? ` (${result.latencyMs} ms)` : "";
  const label = `${result.provider} / ${result.model}`;
  if (result.ok) {
    return {
      type: "success",
      message: `Connection Successful — ${label}${latency}`,
    };
  }
  return {
    type: "error",
    message: `Connection Failed — ${label}: ${result.message}`,
  };
}

export function ThirdPartyModelsPanel({
  backendConnected,
  initialForm = null,
  editingModelId = null,
  onSaved,
}: ThirdPartyModelsPanelProps) {
  const { notify } = useToast();
  const [form, setForm] = useState<ThirdPartyModelForm>(() =>
    initialForm ?? thirdPartyModelTemplate("openai"),
  );
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [successfulTest, setSuccessfulTest] = useState<{
    fingerprint: string;
    latencyMs: number;
  } | null>(null);
  const [catalog, setCatalog] = useState<string[]>([]);
  const [catalogOpen, setCatalogOpen] = useState(false);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const catalogKey = useRef<string | null>(null);
  const catalogInflight = useRef<string | null>(null);
  const catalogRequest = useRef(0);
  const catalogWantsOpen = useRef(false);

  useEffect(() => {
    if (initialForm) {
      setForm(initialForm);
      setSaved(false);
      setError(null);
      setSuccessfulTest(null);
      setCatalog([]);
      setCatalogOpen(false);
      catalogKey.current = null;
    }
  }, [initialForm]);

  const isEditing = Boolean(editingModelId);

  const selectedProvider =
    THIRD_PARTY_PROVIDERS.find((provider) => provider.value === form.provider) ??
    THIRD_PARTY_PROVIDERS[0];
  const providerLabel =
    form.provider === "custom"
      ? form.customProviderName || "Custom"
      : selectedProvider.label;
  const isBedrock = form.provider === "bedrock";
  const isCustom = form.provider === "custom";
  const accessKeyRequiresSessionToken = form.apiKey.trim().startsWith("ASIA");

  function patchForm(patchValue: Partial<ThirdPartyModelForm>) {
    setSaved(false);
    setSuccessfulTest(null);
    if (
      "apiKey" in patchValue ||
      "baseUrl" in patchValue ||
      "apiKeyEnv" in patchValue ||
      "customProviderName" in patchValue ||
      "awsSecretAccessKey" in patchValue ||
      "awsSessionToken" in patchValue ||
      "awsRegion" in patchValue
    ) {
      catalogKey.current = null;
    }
    setForm((current) => ({ ...current, ...patchValue }));
  }

  function selectProvider(provider: ThirdPartyProvider) {
    setForm(thirdPartyModelTemplate(provider));
    setSaved(false);
    setSuccessfulTest(null);
    setCatalog([]);
    setCatalogOpen(false);
    catalogKey.current = null;
  }

  async function loadCatalog(openList: boolean) {
    if (openList) catalogWantsOpen.current = true;
    if (!backendConnected || !catalogReady(form)) {
      setCatalogOpen(catalogWantsOpen.current && catalog.length > 0);
      return;
    }
    const key = catalogFingerprint(form);
    if (catalogKey.current === key || catalogInflight.current === key) {
      setCatalogOpen(
        catalogWantsOpen.current && (catalog.length > 0 || catalogInflight.current === key),
      );
      return;
    }
    const requestId = catalogRequest.current + 1;
    catalogRequest.current = requestId;
    catalogInflight.current = key;
    setCatalogLoading(true);
    if (catalogWantsOpen.current) setCatalogOpen(true);
    try {
      const result = await listProviderModels(form, editingModelId);
      if (catalogRequest.current !== requestId) return;
      catalogKey.current = key;
      setCatalog(result.models);
      setCatalogOpen(catalogWantsOpen.current && result.models.length > 0);
    } catch {
      if (catalogRequest.current !== requestId) return;
      catalogKey.current = key;
      setCatalog([]);
      setCatalogOpen(false);
    } finally {
      if (catalogInflight.current === key) catalogInflight.current = null;
      if (catalogRequest.current === requestId) setCatalogLoading(false);
    }
  }

  async function handleSave() {
    const validationError = validateThirdPartyModelForm(form);
    if (validationError) {
      notify(validationError, "error");
      return;
    }

    const fingerprint = thirdPartyConnectionTestFingerprint(form);
    const priorTest =
      successfulTest !== null && successfulTest.fingerprint === fingerprint
        ? successfulTest
        : null;

    setError(null);
    setSaving(true);
    try {
      const savedForm = await saveThirdPartyModelForm(form, editingModelId, {
        markVerified: priorTest !== null,
        testLatencyMs: priorTest?.latencyMs ?? null,
      });
      setForm(savedForm);
      setSaved(true);
      setSuccessfulTest(null);
      notify(
        priorTest
          ? "Third-party model saved and marked Verified"
          : "Third-party model saved",
        "success",
      );
      if (!editingModelId) {
        recordLocalActivity({
          type: "model",
          message: `Added model: ${savedForm.model || "Third-party model"}`,
        }).catch(() => undefined);
      }
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
      const result = await testThirdPartyModelConnectivity(form, editingModelId);
      const toast = connectivityToast(result);
      notify(toast.message, toast.type);
      if (result.ok) {
        setSuccessfulTest({
          fingerprint: thirdPartyConnectionTestFingerprint(form),
          latencyMs: result.latencyMs,
        });
      } else {
        setSuccessfulTest(null);
      }
    } catch (err) {
      setSuccessfulTest(null);
      notify(toAppError(err).message, "error");
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="third-party-models">
      {isEditing ? (
        <div className="third-party-models__edit-header">
          <span className="third-party-models__edit-label">Provider</span>
          <div className="third-party-models__edit-value-row">
            <span className="third-party-models__icon" aria-hidden="true">
              <ProviderLogo provider={form.provider} />
            </span>
            <strong className="third-party-models__edit-value">{providerLabel}</strong>
          </div>
        </div>
      ) : (
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
                  <ProviderLogo provider={provider.value} />
                </span>
                <span className="third-party-models__name">{provider.label}</span>
              </button>
            );
          })}
        </div>
      )}

      <div className="third-party-models__form">
        {isBedrock ? (
          <>
            <div className="settings-field">
              <ModelIdLabel
                htmlFor="bedrockModel"
                providerLabel={selectedProvider.label}
                modelsDocsUrl={selectedProvider.modelsDocsUrl}
              />
              <ModelIdPicker
                id="bedrockModel"
                value={form.model}
                placeholder={selectedProvider.modelPlaceholder}
                disabled={!backendConnected}
                mono
                loading={catalogLoading}
                open={catalogOpen}
                options={catalog}
                onChange={(model) => {
                  patchForm({ model });
                  setCatalogOpen(true);
                }}
                onFocus={() => void loadCatalog(true)}
                onBlur={() => {
                  catalogWantsOpen.current = false;
                  window.setTimeout(() => setCatalogOpen(false), 120);
                }}
                onPick={(model) => {
                  catalogWantsOpen.current = false;
                  patchForm({ model });
                  setCatalogOpen(false);
                }}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="bedrockAccessKeyId">Access Key ID</label>
              <input
                id="bedrockAccessKeyId"
                className="input mono"
                placeholder="AKIA... or ASIA..."
                value={form.apiKey}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ apiKey: e.target.value })}
                onBlur={() => void loadCatalog(false)}
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
                onBlur={() => void loadCatalog(false)}
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
                onBlur={() => void loadCatalog(false)}
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
                onBlur={() => void loadCatalog(false)}
              />
            </div>
          </>
        ) : (
          <>
            {isCustom && !isEditing && (
              <>
                <p className="text-muted text-sm third-party-models__custom-hint">
                  OpenAI-compatible API with <code>/chat/completions</code>. Use the base URL of your
                  gateway or self-hosted server (for example <code>https://host/v1</code>).
                </p>
                <div className="settings-field">
                  <label htmlFor="thirdPartyCustomProvider">Provider</label>
                  <input
                    id="thirdPartyCustomProvider"
                    className="input"
                    placeholder="lm-studio"
                    value={form.customProviderName}
                    disabled={!backendConnected}
                    onChange={(e) => patchForm({ customProviderName: e.target.value })}
                  />
                </div>
              </>
            )}

            <div className="settings-field">
              <ModelIdLabel
                htmlFor="thirdPartyModel"
                providerLabel={selectedProvider.label}
                modelsDocsUrl={selectedProvider.modelsDocsUrl}
              />
              <ModelIdPicker
                id="thirdPartyModel"
                value={form.model}
                placeholder={selectedProvider.modelPlaceholder}
                disabled={!backendConnected}
                loading={catalogLoading}
                open={catalogOpen}
                options={catalog}
                onChange={(model) => {
                  patchForm({ model });
                  setCatalogOpen(catalog.length > 0);
                }}
                onFocus={() => void loadCatalog(true)}
                onBlur={() => {
                  catalogWantsOpen.current = false;
                  window.setTimeout(() => setCatalogOpen(false), 120);
                }}
                onPick={(model) => {
                  catalogWantsOpen.current = false;
                  patchForm({ model });
                  setCatalogOpen(false);
                }}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyBaseUrl">
                {isCustom
                  ? "Base URL"
                  : selectedProvider.requiresBaseUrl
                    ? "Endpoint URL"
                    : "Custom Base URL (optional)"}
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
                onBlur={() => void loadCatalog(false)}
              />
            </div>

            <div className="settings-field">
              <label htmlFor="thirdPartyApiKey">API Key (optional)</label>
              <input
                id="thirdPartyApiKey"
                className="input mono"
                type="password"
                placeholder={form.apiKeyConfigured ? "Configured in keychain" : "sk-..."}
                value={form.apiKey}
                disabled={!backendConnected}
                onChange={(e) => patchForm({ apiKey: e.target.value })}
                onBlur={() => void loadCatalog(false)}
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

      <div className="third-party-models__sampling">
        <div className="settings-field">
          <label htmlFor="modelTemperature">Temperature</label>
          <input
            id="modelTemperature"
            className="input"
            type="number"
            min={0}
            max={2}
            step={0.1}
            value={form.temperature}
            disabled={!backendConnected}
            onChange={(e) => {
              const next = Number(e.target.value);
              if (Number.isFinite(next)) patchForm({ temperature: next });
            }}
          />
        </div>
        <div className="settings-field">
          <label htmlFor="modelContextWindow">Context window size</label>
          <input
            id="modelContextWindow"
            className="input"
            type="number"
            min={1}
            step={1}
            value={form.contextWindow}
            disabled={!backendConnected}
            onChange={(e) => {
              const next = Number(e.target.value);
              if (Number.isFinite(next)) patchForm({ contextWindow: Math.trunc(next) });
            }}
          />
        </div>
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
          {saving ? "Saving…" : saved ? "Saved" : isEditing ? "Save changes" : "Save config"}
        </Button>
      </div>

      {error && <p className="text-danger text-sm">{error}</p>}
    </div>
  );
}
