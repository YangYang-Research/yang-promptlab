import { useMemo } from "react";

import {
  formatExecutionStrategySummary,
  type AttackPlanConfig,
} from "@/features/scans/attackPlan";
import {
  executionPipelineLiveDetail,
  phaseLabel,
  resolveExecutionTrail,
  type ExecutionStepState,
} from "@/features/scans/executionStrategyPipeline";
import { Badge } from "@/shared/components";
import type { ScanStatusDto } from "@/shared/ipc";

type ExecutionStrategyPipelineProps = {
  attackPlan: AttackPlanConfig;
  status?: ScanStatusDto | null;
  compact?: boolean;
};

function stepMarker(state: ExecutionStepState, index: number): string {
  if (state === "done") return "✓";
  if (state === "failed") return "!";
  return String(index + 1);
}

export function ExecutionStrategyPipeline({
  attackPlan,
  status = null,
  compact = false,
}: ExecutionStrategyPipelineProps) {
  const liveTrail = useMemo(() => resolveExecutionTrail(status), [status]);
  const liveDetail = executionPipelineLiveDetail(status);
  const hasLiveStatus = Boolean(status && status.status !== "draft");
  const showLiveTrail = hasLiveStatus && liveTrail.length > 0;

  return (
    <section className="wizard-fingerprint-summary">
      <div className="wizard-planner-summary-header">
        <h4 className="wizard-endpoints__title">Execution pipeline</h4>
        <Badge variant="info">{formatExecutionStrategySummary(attackPlan)}</Badge>
      </div>

      {showLiveTrail ? (
        <ol
          className={`wizard-execution-pipeline wizard-execution-pipeline--live${compact ? " wizard-execution-pipeline--compact" : ""}`}
        >
          {liveTrail.map((step, index) => (
            <li
              key={step.id}
              className={[
                "wizard-execution-pipeline__step",
                `wizard-execution-pipeline__step--${step.state}`,
              ].join(" ")}
            >
              <span className="wizard-execution-pipeline__marker" aria-hidden>
                {stepMarker(step.state, index)}
              </span>
              <div className="wizard-execution-pipeline__body">
                <span className="wizard-execution-pipeline__label-row">
                  <span className="wizard-execution-pipeline__label">{step.label}</span>
                </span>
              </div>
            </li>
          ))}
        </ol>
      ) : hasLiveStatus ? (
        <p className="wizard-execution-pipeline__live text-sm text-muted">
          {status?.current_phase
            ? `${phaseLabel(status.current_phase)} in progress…`
            : "Waiting for the first pipeline stage…"}
        </p>
      ) : (
        <p className="text-sm text-muted">
          Stages appear as the attack runs — e.g. Generate → Attack · Jailbreak → Judge · Jailbreak.
        </p>
      )}

      {!compact && liveDetail && (
        <p className="wizard-execution-pipeline__live text-sm text-muted">{liveDetail}</p>
      )}
    </section>
  );
}
