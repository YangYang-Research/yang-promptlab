import { IconAi } from "@/shared/components/Icons";
import { Skeleton } from "@/shared/components/Skeleton";

type AttackPlanPlanningStateProps = {
  replanning: boolean;
};

export function AttackPlanPlanningState({ replanning }: AttackPlanPlanningStateProps) {
  const title = replanning ? "Re-planning" : "Planning";
  const detail = replanning
    ? "Yazg is regenerating your attack plan…"
    : "Yazg is building your attack plan…";

  return (
    <div
      className={`wizard-attack-planning${replanning ? " wizard-attack-planning--replan" : ""}`}
      aria-busy="true"
      aria-live="polite"
    >
      <div className="wizard-attack-planning__header">
        <span className="wizard-attack-planning__spinner page-loader__spinner" aria-hidden />
        <div>
          <p className="wizard-attack-planning__title">
            <IconAi className="wizard-attack-planning__icon" aria-hidden />
            {title}
          </p>
          <p className="wizard-attack-planning__detail text-muted text-sm">{detail}</p>
        </div>
      </div>

      <div className="wizard-attack-planning__skeleton" aria-hidden>
        <div className="wizard-attack-planning__skeleton-row">
          <Skeleton width="7rem" height="0.75rem" />
          <Skeleton width="4.5rem" height="1rem" />
        </div>
        <div className="wizard-attack-planning__skeleton-grid">
          {Array.from({ length: 4 }).map((_, index) => (
            <div key={index} className="wizard-attack-planning__skeleton-card">
              <Skeleton width="55%" height="0.75rem" />
              <Skeleton width="80%" height="0.625rem" />
              <Skeleton width="65%" height="0.625rem" />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
