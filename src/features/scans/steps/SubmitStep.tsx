import { useMemo } from "react";
import { Link } from "react-router-dom";

import { Badge, Button } from "@/shared/components";
import {
  ATTACK_PROFILES,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  type AttackPlanConfig,
} from "@/features/scans/attackProfiles";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import type { Target } from "@/shared/types";

type SubmitStepProps = {
  target: Target;
  endpointIds: string[];
  attackPlan: AttackPlanConfig;
  submittedScanId: string | null;
  onCreateAnother: () => void;
};

export function SubmitStep({
  target,
  endpointIds,
  attackPlan,
  submittedScanId,
  onCreateAnother,
}: SubmitStepProps) {
  const statuses = useScanStatuses(submittedScanId ? [submittedScanId] : [], submittedScanId !== null);
  const liveStatus = submittedScanId ? statuses.get(submittedScanId) : undefined;
  const status = submittedScanId
    ? mergeScanStatus(submittedScanId, "running", liveStatus, 0)
    : null;

  const estimateInput = useMemo(
    () => ({
      selectedEndpointCount: endpointIds.length,
      profileId: attackPlan.profileId,
      customCategories: attackPlan.customCategories,
      disabledTestIds: new Set(attackPlan.disabledTests),
    }),
    [attackPlan, endpointIds.length],
  );

  const summaryRows = useMemo(() => {
    const profileLabel =
      ATTACK_PROFILES.find((profile) => profile.id === attackPlan.profileId)?.label ??
      attackPlan.profileId;
    return [
      { label: "Target", value: target.url },
      { label: "Endpoints", value: String(endpointIds.length) },
      { label: "Profile", value: profileLabel },
      { label: "Categories", value: attackPlan.categories.join(", ") },
      {
        label: "Est. requests",
        value: estimateRequests(estimateInput).toLocaleString(),
      },
      {
        label: "Est. runtime",
        value: formatEstimatedRuntime(estimateRuntimeSeconds(estimateInput)),
      },
    ];
  }, [attackPlan, endpointIds.length, estimateInput, target.url]);

  if (submittedScanId && status) {
    return (
      <div className="wizard-submitted">
        <div className="wizard-submitted__hero">
          <h3 className="wizard-submitted__title">Scan execution summary</h3>
          <p className="text-muted">
            The scan is running in the background. Monitor progress here or jump to related views.
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

        <div className="wizard-submitted__actions">
          <Link to="/scans">
            <Button variant="primary">Open Scan Monitor</Button>
          </Link>
          <Button variant="secondary" onClick={onCreateAnother}>
            Create Another Scan
          </Button>
          <Link to={`/findings?scanId=${encodeURIComponent(submittedScanId)}`}>
            <Button variant="secondary">Go To Findings</Button>
          </Link>
          <Link to="/targets">
            <Button variant="ghost">Go To Targets</Button>
          </Link>
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
