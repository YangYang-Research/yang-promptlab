import { useNavigate } from "react-router-dom";

import { Card, IconArrowRight, IconWarning } from "@/shared/components";
import type { RuntimeConfigurationDto } from "@/shared/ipc/runtime";

type AiRuntimeDashboardCardProps = {
  configuration: RuntimeConfigurationDto | null;
  loading?: boolean;
};

export function AiRuntimeDashboardCard({ configuration, loading }: AiRuntimeDashboardCardProps) {
  const navigate = useNavigate();

  const mode = configuration?.mode ?? "not_configured";

  function modeLabel(): string {
    if (mode === "third_party") return "Third-party";
    if (mode === "local") return "Local Runtime";
    return "—";
  }

  return (
    <button
      type="button"
      className="stat-card-button"
      onClick={() => navigate("/runtime")}
      aria-label="Open AI Runtime settings"
    >
      <Card className="stat-card stat-card--runtime">
        <span className="stat-card__label">AI Runtime</span>

        {loading ? (
          <span className="stat-card__value stat-card__value--sm">Loading…</span>
        ) : mode === "not_configured" ? (
          <span className="stat-card__setup-row">
            <IconWarning className="stat-card__setup-icon" />
            <span className="stat-card__value stat-card__value--setup">Setup Required</span>
            <IconArrowRight className="stat-card__setup-arrow" />
          </span>
        ) : (
          <div className="runtime-dashboard-card__body">
            <div className="runtime-dashboard-card__row">
              <span className="runtime-dashboard-card__key">Mode</span>
              <span className="runtime-dashboard-card__val">{modeLabel()}</span>
            </div>
            <div className="runtime-dashboard-card__row">
              <span className="runtime-dashboard-card__key">Status</span>
              <span className="runtime-dashboard-card__val">{configuration?.statusLabel ?? "—"}</span>
            </div>
            {mode === "local" && (
              <>
                <div className="runtime-dashboard-card__row">
                  <span className="runtime-dashboard-card__key">Runtime</span>
                  <span className="runtime-dashboard-card__val">
                    {configuration?.runtimeName ?? "—"}
                  </span>
                </div>
                <div className="runtime-dashboard-card__row">
                  <span className="runtime-dashboard-card__key">Model</span>
                  <span className="runtime-dashboard-card__val">
                    {configuration?.modelName ?? "—"}
                  </span>
                </div>
              </>
            )}
            {mode === "third_party" && (
              <div className="runtime-dashboard-card__row">
                <span className="runtime-dashboard-card__key">Provider</span>
                <span className="runtime-dashboard-card__val">
                  {configuration?.provider ?? "—"}
                </span>
              </div>
            )}
          </div>
        )}
      </Card>
    </button>
  );
}
