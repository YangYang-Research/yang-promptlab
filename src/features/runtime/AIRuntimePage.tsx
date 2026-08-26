import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  Button,
  Card,
  ConnectivityStatus,
  connectivityStatusVariant,
  EmptyState,
  PageHeader,
  PageLoadingSkeleton,
  RefreshButton,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { recordLocalActivity } from "@/shared/activity/localActivity";
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import { isYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import { type AiInferenceModelOptionDto } from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";

import { RegistryProviderIcon } from "@/features/models/ProviderLogo";
import { RuntimeTrafficChart } from "@/features/runtime/RuntimeTrafficChart";

export function AIRuntimePage() {
  const { notify } = useToast();
  const navigate = useNavigate();
  const [backendConnected, setBackendConnected] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [testingModelId, setTestingModelId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const {
    configuration,
    settings,
    loading: configLoading,
    busy: routeBusy,
    error: configError,
    refresh: refreshConfiguration,
    setRoute,
  } = useAiInferenceRoute({ enabled: backendConnected });

  const refreshAll = useCallback(async () => {
    await refreshConfiguration();
  }, [refreshConfiguration]);

  useEffect(() => {
    void import("@/shared/ipc/client").then(({ healthCheck }) =>
      healthCheck()
        .then(() => setBackendConnected(true))
        .catch(() => setBackendConnected(false))
        .finally(() => setLoading(false)),
    );
  }, []);

  // Persist remote-only defaults for leftover unset configs.
  useEffect(() => {
    if (!backendConnected || configLoading || !configuration) return;
    if (configuration.mode === "not_configured") {
      void setRoute("third_party").catch(() => undefined);
    }
  }, [backendConnected, configLoading, configuration, setRoute]);

  async function handleRefresh() {
    if (!backendConnected || refreshing) return;
    setRefreshing(true);
    setError(null);
    try {
      await refreshAll();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setRefreshing(false);
    }
  }

  async function handleSelectThirdPartyModel(modelId: string) {
    if (!backendConnected || loading || routeBusy || testingModelId !== null) {
      return;
    }
    setError(null);
    setTestingModelId(modelId);
    try {
      const result = await setRoute("third_party", modelId);
      const modelName =
        (settings?.thirdPartyModels ?? []).find((model) => model.id === modelId)?.name ?? modelId;
      recordLocalActivity({
        type: "runtime",
        message: `Selected AI Runtime model: ${modelName}`,
      }).catch(() => undefined);
      if (result?.settings.connectivityTestOk === false && result.settings.connectivityTestDetail) {
        notify(result.settings.connectivityTestDetail, "error");
      }
    } catch (err) {
      const message = toAppError(err).message;
      notify(message, "error");
    } finally {
      setTestingModelId(null);
    }
  }

  const disabled = !backendConnected || loading || routeBusy;
  const thirdPartyModels = settings?.thirdPartyModels ?? [];
  const connectionDotVariant = testingModelId
    ? null
    : connectivityStatusVariant(configuration?.connectivity);

  return (
    <div className="page runtime-page">
      <PageHeader
        title="AI Runtime"
        description="Manage the remote AI Runtime for Yazg Agent"
        actions={
          backendConnected ? (
            <RefreshButton
              loading={refreshing || configLoading || testingModelId !== null}
              error={error}
              disabled={!backendConnected}
              onClick={() => void handleRefresh()}
            />
          ) : undefined
        }
      />

      {error ? (
        <div className="runtime-page__alert" role="alert">
          {error}
        </div>
      ) : null}

      {!backendConnected ? (
        <Card className="runtime-page__banner detail-section">
          <p className="runtime-page__banner-text">
            Connect to the Tauri backend to manage the AI runtime.
          </p>
        </Card>
      ) : null}

      {configLoading && !configuration && backendConnected ? <PageLoadingSkeleton /> : null}

      {configError && !configuration && backendConnected && !configLoading ? (
        <div className="runtime-page__alert" role="alert">
          {configError}
        </div>
      ) : null}

      {backendConnected && configuration ? (
        <>
          <section className="runtime-page__overview" aria-label="Runtime overview">
            <Card className="detail-section runtime-page__status">
              <h2 className="detail-section__title">Status</h2>
              <div className="detail-summary-grid detail-summary-grid--metrics">
                <div className="summary-stat">
                  <span className="summary-stat__label">Runtime status</span>
                  <span className="summary-stat__value summary-stat__value--sm">
                    {configuration.statusLabel ? (
                      <ConnectivityStatus label={configuration.statusLabel} />
                    ) : (
                      "N/A"
                    )}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="summary-stat__label">Provider</span>
                  <span className="summary-stat__value summary-stat__value--sm">
                    {configuration.provider ? (
                      <RegistryProviderIcon provider={configuration.provider} />
                    ) : (
                      "N/A"
                    )}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="summary-stat__label">Model ID</span>
                  <span className="summary-stat__value summary-stat__value--sm">
                    {configuration.modelName ?? settings?.selectedModelName ?? "N/A"}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="summary-stat__label">Yazg Agent</span>
                  <span className="summary-stat__value summary-stat__value--sm">
                    {testingModelId ? (
                      "Checking…"
                    ) : (
                      <ConnectivityStatus
                        label={isYazgAgentLive(configuration) ? "Live" : "Offline"}
                      />
                    )}
                  </span>
                </div>
              </div>
            </Card>

            <Card className="detail-section runtime-page__meta">
              <h2 className="detail-section__title runtime-page__connection-title">
                Connection
                {connectionDotVariant ? (
                  <span
                    className={`connectivity-status__dot connectivity-status__dot--${connectionDotVariant}`}
                    aria-hidden
                  />
                ) : null}
              </h2>
              <dl className="runtime-page__meta-list">
                <div>
                  <dt>Connectivity</dt>
                  <dd>
                    {testingModelId ? "Testing…" : configuration.connectivity ?? "N/A"}
                  </dd>
                </div>
                <div>
                  <dt>Last health check</dt>
                  <dd>
                    {testingModelId
                      ? "Running connection test…"
                      : configuration.lastHealthCheck ?? "Not checked"}
                  </dd>
                </div>
              </dl>
            </Card>
          </section>

          <Card className="detail-section runtime-page__traffic">
            <h2 className="detail-section__title">Traffic monitor</h2>
            <RuntimeTrafficChart enabled={backendConnected} defaultRangeId="1m" />
          </Card>

          <section className="runtime-page__primary" aria-label="Registered models">
            <Card className="detail-section">
              <div className="detail-section__header">
                <div>
                  <h2 className="detail-section__title">Registered models</h2>
                  <p className="detail-section__hint">
                    {thirdPartyModels.length === 0
                      ? "Add a remote provider model to route AI requests."
                      : `${thirdPartyModels.length} registered model${thirdPartyModels.length === 1 ? "" : "s"}`}
                  </p>
                </div>
                <div className="detail-section__header-actions">
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() =>
                      navigate("/models", {
                        state: { openAddModel: true, openAddModelTab: "third-party" },
                      })
                    }
                  >
                    Add Model
                  </Button>
                </div>
              </div>

              {thirdPartyModels.length > 0 ? (
                <ul className="runtime-route-models" aria-label="Registered remote models">
                  {thirdPartyModels.map((model) => (
                    <ThirdPartyModelRow
                      key={model.id}
                      model={model}
                      selected={model.id === settings?.selectedModelId}
                      disabled={disabled}
                      testing={testingModelId === model.id}
                      onSelect={() => void handleSelectThirdPartyModel(model.id)}
                      onEdit={() =>
                        navigate("/models", { state: { editModelId: model.id } })
                      }
                    />
                  ))}
                </ul>
              ) : (
                <EmptyState
                  title="No remote models yet"
                  description="Register OpenAI, Anthropic, Gemini, Azure, Bedrock, OpenRouter, Ollama HTTP, or a custom endpoint from Models."
                />
              )}
            </Card>
          </section>
        </>
      ) : null}
    </div>
  );
}

function thirdPartyModelNeedsEdit(model: AiInferenceModelOptionDto): boolean {
  return !model.configured;
}

function ThirdPartyModelRow({
  model,
  selected,
  disabled,
  testing,
  onSelect,
  onEdit,
}: {
  model: AiInferenceModelOptionDto;
  selected: boolean;
  disabled: boolean;
  testing: boolean;
  onSelect: () => void;
  onEdit: () => void;
}) {
  const needsEdit = thirdPartyModelNeedsEdit(model);
  return (
    <li
      className={`runtime-route-models__item${selected ? " runtime-route-models__item--selected" : ""}`}
    >
      <div className="runtime-route-models__info">
        <div className="runtime-route-models__name">{model.name}</div>
        <div className="runtime-route-models__meta">
          <RegistryProviderIcon provider={model.provider} />
          <span>{model.statusLabel}</span>
        </div>
      </div>
      {needsEdit ? (
        <Button
          variant="secondary"
          size="sm"
          className="runtime-route-models__pick"
          disabled={disabled || testing}
          onClick={onEdit}
        >
          Edit
        </Button>
      ) : (
        <Button
          variant={selected ? "primary" : "secondary"}
          size="sm"
          className="runtime-route-models__pick"
          disabled={disabled || testing || selected}
          onClick={onSelect}
        >
          {testing ? "Testing…" : selected ? "Selected" : "Use"}
        </Button>
      )}
    </li>
  );
}
