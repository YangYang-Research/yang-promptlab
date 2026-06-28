import { useMemo } from "react";

import { Badge, Button } from "@/shared/components";
import {
  ATTACK_PROFILES,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  type AttackPlanConfig,
} from "@/features/scans/attackProfiles";
import type { TargetProfileFormState } from "@/features/scans/targetProfile";
import { fullProfileUrl, PROVIDER_OPTIONS } from "@/features/scans/targetProfile";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import type { Target } from "@/shared/types";

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

  const estimateInput = useMemo(
    () => ({
      selectedEndpointCount: 1,
      profileId: attackPlan.profileId,
      customCategories: attackPlan.customCategories,
      disabledTestIds: new Set(attackPlan.disabledTests),
    }),
    [attackPlan],
  );

  const summaryRows = useMemo(() => {
    const profileLabel =
      ATTACK_PROFILES.find((profile) => profile.id === attackPlan.profileId)?.label ??
      attackPlan.profileId;
    const providerLabel =
      PROVIDER_OPTIONS.find((p) => p.id === targetProfile.provider)?.label ?? targetProfile.provider;
    return [
      { label: "Target", value: fullProfileUrl(targetProfile) || target.url },
      { label: "AI Platform", value: providerLabel },
      { label: "Profile", value: profileLabel },
      { label: "Categories", value: attackPlan.categories.join(", ") },
      {
        label: "Mode",
        value: attackPlan.agentMode
          ? `Agentic (max ${attackPlan.maxAgentAttempts} attempts/category)`
          : "Batch",
      },
      {
        label: "Est. requests",
        value: estimateRequests(estimateInput).toLocaleString(),
      },
      {
        label: "Est. runtime",
        value: formatEstimatedRuntime(estimateRuntimeSeconds(estimateInput)),
      },
    ];
  }, [attackPlan, estimateInput, target.url, targetProfile]);

  if (submittedScanId && status) {
    const isRunning = ["running", "paused", "pending"].includes(status.status);
    const isSuccess = status.status === "completed";
    const isFailed =
      status.status === "failed" ||
      status.status === "stopped" ||
      status.status === "cancelled";

    return (
      <div className="wizard-submitted">
        <div className="wizard-submitted__hero">
          <h3 className="wizard-submitted__title">Scan progress</h3>
          <p className="text-muted">
            Live output from the attack engine. Monitor progress here or open the scan monitor.
          </p>
        </div>

        <dl className="wizard-submitted__meta">
          <div>
            <dt>Scan ID</dt>
            <dd>
              <code>{submittedScanId}</code>
            </dd>
          </div>
          <div>
            <dt>Status</dt>
            <dd>
              <Badge variant={statusBadgeVariant(status.status)}>{status.status}</Badge>
            </dd>
          </div>
          <div>
            <dt>Progress</dt>
            <dd>
              {status.progress_percent}% ({status.completed}/{status.total || "—"} tests)
            </dd>
          </div>
          {status.current_endpoint && (
            <div>
              <dt>Current endpoint</dt>
              <dd className="text-muted">{status.current_endpoint}</dd>
            </div>
          )}
          {status.current_test && (
            <div>
              <dt>Current test</dt>
              <dd className="text-muted">{status.current_test}</dd>
            </div>
          )}
          <div>
            <dt>Findings</dt>
            <dd>{status.findings_count}</dd>
          </div>
        </dl>

        <div className="wizard-submitted__progress">
          <div
            className="wizard-submitted__progress-bar"
            style={{ width: `${Math.min(100, status.progress_percent)}%` }}
          />
        </div>

        <hr className="wizard-submitted__divider" />

        <ScanConsole scanId={submittedScanId} />

        <div className="wizard-submitted__actions">
          {isSuccess && (
            <Button variant="primary" onClick={onViewResult}>
              View Result
            </Button>
          )}
          {isFailed && (
            <Button variant="primary" onClick={onRetryScan}>
              Retry Scan
            </Button>
          )}
          {isRunning && (
            <span className="text-muted text-sm">Scan running…</span>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="wizard-submit-review">
      <p className="text-muted">
        Review your configuration, then click <strong>Start Scan</strong> below. The job runs in the
        background — you will not be blocked on this screen.
      </p>

      <dl className="wizard-submit-review__grid">
        {summaryRows.map((row) => (
          <div key={row.label}>
            <dt>{row.label}</dt>
            <dd>{row.value}</dd>
          </div>
        ))}
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
