import { useNavigate } from "react-router-dom";

import {
  Card,
  ConnectivityStatus,
  connectivityStatusVariant,
  IconArrowRight,
  IconWarning,
} from "@/shared/components";
import type { RuntimeConfigurationDto } from "@/shared/ipc/runtime";
import { isYazgAgentLive } from "@/shared/runtime/yazgAgentLive";

type AiRuntimeDashboardCardProps = {
  configuration: RuntimeConfigurationDto | null;
  loading?: boolean;
};

export function AiRuntimeDashboardCard({ configuration, loading }: AiRuntimeDashboardCardProps) {
  const navigate = useNavigate();

  const selectedModelId = configuration?.settings.selectedModelId ?? null;
  const needsSetup = !selectedModelId;
  const yazgLive = isYazgAgentLive(configuration);
  const statusDotVariant =
    needsSetup || loading
      ? null
      : connectivityStatusVariant(configuration?.connectivity) ??
        connectivityStatusVariant(configuration?.statusLabel);

  return (
    <button
      type="button"
      className="stat-card-button"
      onClick={() => navigate("/runtime")}
      aria-label="Open AI Runtime settings"
    >
      <Card className="stat-card stat-card--runtime">
        <span className="stat-card__label runtime-dashboard-card__label">
          AI Runtime
          {statusDotVariant ? (
            <span
              className={`connectivity-status__dot connectivity-status__dot--${statusDotVariant}`}
              aria-hidden
            />
          ) : null}
        </span>

        {loading ? (
          <span className="stat-card__value stat-card__value--sm">Loading…</span>
        ) : needsSetup ? (
          <span className="stat-card__setup-row">
            <IconWarning className="stat-card__setup-icon" />
            <span className="stat-card__value stat-card__value--setup">Setup Required</span>
            <IconArrowRight className="stat-card__setup-arrow" />
          </span>
        ) : (
          <div className="runtime-dashboard-card__body">
            <div className="runtime-dashboard-card__row">
              <span className="runtime-dashboard-card__key">Provider</span>
              <span className="runtime-dashboard-card__val">
                {configuration?.provider ?? "—"}
              </span>
            </div>
            <div className="runtime-dashboard-card__row">
              <span className="runtime-dashboard-card__key">Model</span>
              <span className="runtime-dashboard-card__val">
                {configuration?.modelName ?? configuration?.settings.selectedModelName ?? "—"}
              </span>
            </div>
            <div className="runtime-dashboard-card__row">
              <span className="runtime-dashboard-card__key">Yazg Agent</span>
              <span className="runtime-dashboard-card__val">
                <ConnectivityStatus label={yazgLive ? "Live" : "Offline"} />
              </span>
            </div>
          </div>
        )}
      </Card>
    </button>
  );
}
