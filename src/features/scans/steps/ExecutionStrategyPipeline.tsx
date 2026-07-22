import { useMemo } from "react";

import type { AttackPlanConfig } from "@/features/scans/attackPlan";
import {
  executionPipelineLiveDetail,
  executionStrategySteps,
  executionStrategyTitle,
  resolveExecutionStepStates,
  type ExecutionStepState,
} from "@/features/scans/executionStrategyPipeline";
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
  const steps = useMemo(() => executionStrategySteps(attackPlan), [attackPlan]);
  const states = useMemo(
    () => resolveExecutionStepStates(steps, status),
    [steps, status],
  );
  const liveDetail = executionPipelineLiveDetail(status);

  return (
    <section className="wizard-fingerprint-summary">
      <div className="wizard-planner-summary-header">
        <h4 className="wizard-endpoints__title">Execution pipeline</h4>
        <span className="text-sm text-muted">{executionStrategyTitle(attackPlan)}</span>
      </div>

      <ol
        className={`wizard-execution-pipeline${compact ? " wizard-execution-pipeline--compact" : ""}`}
      >
        {steps.map((step, index) => {
          const state = states[index] ?? "pending";
          return (
            <li
              key={step.id}
              className={[
                "wizard-execution-pipeline__step",
                state !== "pending" ? `wizard-execution-pipeline__step--${state}` : "",
              ]
                .filter(Boolean)
                .join(" ")}
            >
              <span className="wizard-execution-pipeline__marker" aria-hidden>
                {stepMarker(state, index)}
              </span>
              <div className="wizard-execution-pipeline__body">
                <span className="wizard-execution-pipeline__label-row">
                  <span className="wizard-execution-pipeline__label">{step.label}</span>
                </span>
                {!compact && (
                  <span className="wizard-execution-pipeline__description text-sm text-muted">
                    {step.description}
                  </span>
                )}
              </div>
            </li>
          );
        })}
      </ol>

      {!compact && liveDetail && (
        <p className="wizard-execution-pipeline__live text-sm text-muted">{liveDetail}</p>
      )}
    </section>
  );
}
