import { useMemo } from "react";

import { Badge, Button } from "@/shared/components";
import {
  formatCoverageScore,
  formatEstimatedRuntime,
  formatExecutionStrategySummary,
  type AttackPlanConfig,
} from "@/features/scans/attackPlan";
import { formatPayloadStrategySummary } from "@/features/scans/payloadStrategy";
import { ATTACK_PROFILES, getCategory } from "@/features/scans/attackProfiles";
import type { TargetProfileFormState } from "@/features/scans/targetProfile";
import { fullProfileUrl, PROVIDER_OPTIONS } from "@/features/scans/targetProfile";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import type { Target } from "@/shared/types";

import { AttackGraphProgress } from "./AttackGraphProgress";
import { ExecutionStrategyPipeline } from "./ExecutionStrategyPipeline";
import { ScanConsole } from "./ScanConsole";

type SubmitStepProps = {
  target: Target;
  targetProfile: TargetProfileFormState;
  attackPlan: AttackPlanConfig;
  submittedScanId: string | null;
  onViewResult: () => void;
  onRetryScan: () => void;
};

export function SubmitStep({
  target,
  targetProfile,
  attackPlan,
  submittedScanId,
  onViewResult,
  onRetryScan,
}: SubmitStepProps) {
  const statuses = useScanStatuses(submittedScanId ? [submittedScanId] : [], submittedScanId !== null);
  const liveStatus = submittedScanId ? statuses.get(submittedScanId) : undefined;
  const status = submittedScanId
    ? mergeScanStatus(submittedScanId, "running", liveStatus, 0)
    : null;

  const profileLabel =
    ATTACK_PROFILES.find((profile) => profile.id === attackPlan.profileId)?.label ??
    attackPlan.profileId;
  const providerLabel =
    PROVIDER_OPTIONS.find((p) => p.id === targetProfile.provider)?.label ?? targetProfile.provider;
  const targetUrl = fullProfileUrl(targetProfile) || target.url;
  const executionLabel = useMemo(() => {
    if (attackPlan.executionStrategy === "agentic") {
      return `Agentic · up to ${attackPlan.maxAttempts} attempts/category`;
    }
    return formatExecutionStrategySummary(attackPlan);
  }, [attackPlan]);

  const attackGraphSection = (
    <section className="wizard-fingerprint-summary">
      <div className="wizard-planner-summary-header">
        <h4 className="wizard-endpoints__title">Attack graph</h4>
        {status ? (
          <span className="text-sm text-muted">
            {status.completed}/{status.total || attackPlan.categories.length} categories
          </span>
        ) : (
          <span className="text-sm text-muted">{attackPlan.categories.length} categories queued</span>
        )}
      </div>
      <AttackGraphProgress categories={attackPlan.categories} status={status} />
    </section>
  );

  if (submittedScanId && status) {
    const isRunning = ["running", "paused", "pending"].includes(status.status);
    const isSuccess = status.status === "completed";
    const isFailed =
      status.status === "failed" ||
      status.status === "stopped" ||
      status.status === "cancelled";
    const statusTitle = isSuccess
      ? "Attack complete"
      : isFailed
        ? "Attack stopped"
        : status.status === "paused"
          ? "Attack paused"
          : "Attack in progress";

    return (
      <div className="wizard-step wizard-submitted">
        <section className="wizard-fingerprint-summary">
          <div className="wizard-planner-summary-header">
            <h4 className="wizard-endpoints__title">{statusTitle}</h4>
            <Badge variant={statusBadgeVariant(status.status)}>{status.status}</Badge>
          </div>

          <div className="wizard-submitted__progress">
            <div
              className="wizard-submitted__progress-bar"
              style={{ width: `${Math.min(100, status.progress_percent)}%` }}
            />
          </div>

          <dl className="wizard-attack-estimates wizard-attack-estimates--compact">
            <div className="wizard-attack-estimate">
              <span className="wizard-attack-estimate__label">Progress</span>
              <span className="wizard-attack-estimate__value">{status.progress_percent}%</span>
            </div>
            <div className="wizard-attack-estimate">
              <span className="wizard-attack-estimate__label">Categories</span>
              <span className="wizard-attack-estimate__value">
                {status.completed}/{status.total || "—"}
              </span>
            </div>
            <div className="wizard-attack-estimate">
              <span className="wizard-attack-estimate__label">Findings</span>
              <span className="wizard-attack-estimate__value">{status.findings_count}</span>
            </div>
            <div className="wizard-attack-estimate">
              <span className="wizard-attack-estimate__label">Run ID</span>
              <span className="wizard-attack-estimate__value wizard-attack-estimate__value--mono">
                {submittedScanId.slice(0, 8)}
              </span>
            </div>
          </dl>
        </section>

        <ExecutionStrategyPipeline attackPlan={attackPlan} status={status} compact />

        {attackGraphSection}

        <section className="wizard-fingerprint-summary">
          <h4 className="wizard-endpoints__title">Live log</h4>
          <ScanConsole scanId={submittedScanId} />
        </section>

        <div className="wizard-submitted__actions">
          {isSuccess && (
            <Button variant="primary" onClick={onViewResult}>
              View results
            </Button>
          )}
          {isFailed && (
            <Button variant="primary" onClick={onRetryScan}>
              Retry attack
            </Button>
          )}
          {isRunning && <span className="text-muted text-sm">Attack running in background…</span>}
        </div>
      </div>
    );
  }

  return (
    <div className="wizard-step wizard-submit-review">
      <p className="wizard-submit-review__lead text-sm text-muted">
        Review the attack graph below. Press <strong>Start Attack</strong> in the footer to launch,
        or use <strong>Start Attack</strong> from the previous step.
      </p>

      <section className="wizard-fingerprint-summary">
        <h4 className="wizard-endpoints__title">Target</h4>
        <p className="wizard-submit-review__url mono text-sm">{targetUrl}</p>
        <p className="text-sm text-muted">{providerLabel}</p>
      </section>

      <section className="wizard-fingerprint-summary">
        <div className="wizard-planner-summary-header">
          <h4 className="wizard-endpoints__title">Attack plan</h4>
          <Badge variant="info">{profileLabel}</Badge>
        </div>
        <div className="tag-list wizard-submit-review__categories">
          {attackPlan.categories.map((category) => (
            <span key={category} className="chip">
              {getCategory(category).label}
            </span>
          ))}
        </div>
        <p className="wizard-submit-review__plan-meta text-sm text-muted">
          {executionLabel} · {formatPayloadStrategySummary(attackPlan.payloadStrategy)}
        </p>
      </section>

      <ExecutionStrategyPipeline attackPlan={attackPlan} />

      {attackGraphSection}

      <dl className="wizard-attack-estimates">
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Active tests</span>
          <span className="wizard-attack-estimate__value">
            {attackPlan.totalTestcases.toLocaleString()}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Est. runtime</span>
          <span className="wizard-attack-estimate__value">
            {formatEstimatedRuntime(attackPlan.estimatedRuntimeSeconds)}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Est. requests</span>
          <span className="wizard-attack-estimate__value">
            {attackPlan.estimatedRequests.toLocaleString()}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Coverage</span>
          <span className="wizard-attack-estimate__value">
            {formatCoverageScore(attackPlan.coverageScore)}
          </span>
        </div>
      </dl>
    </div>
  );
}

function statusBadgeVariant(status: string): "success" | "warning" | "danger" | "info" | "muted" {
  if (status === "completed") return "success";
  if (status === "running") return "info";
  if (status === "paused") return "warning";
  if (status === "failed" || status === "stopped") return "danger";
  return "muted";
}
