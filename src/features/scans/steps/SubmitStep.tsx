import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Button } from "@/shared/components";
import { startScan } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { Target } from "@/shared/types";

import {
  ATTACK_PROFILES,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  type AttackPlanConfig,
} from "../attackProfiles";

type SubmitStepProps = {
  projectId: string;
  target: Target;
  endpointIds: string[];
  attackPlan: AttackPlanConfig;
};

export function SubmitStep({
  projectId,
  target,
  endpointIds,
  attackPlan,
}: SubmitStepProps) {
  const navigate = useNavigate();
  const { actions } = useAppStore();
  const { notify } = useToast();
  const [submitting, setSubmitting] = useState(false);

  const profileLabel =
    ATTACK_PROFILES.find((p) => p.id === attackPlan.profileId)?.label ?? attackPlan.profileId;

  const estimatedRequests = estimateRequests({
    selectedEndpointCount: endpointIds.length,
    profileId: attackPlan.profileId,
    customCategories: attackPlan.customCategories,
    disabledTestIds: new Set(attackPlan.disabledTests),
  });
  const estimatedRuntime = formatEstimatedRuntime(
    estimateRuntimeSeconds({
      selectedEndpointCount: endpointIds.length,
      profileId: attackPlan.profileId,
      customCategories: attackPlan.customCategories,
      disabledTestIds: new Set(attackPlan.disabledTests),
    }),
  );

  async function handleStartScan() {
    if (submitting || attackPlan.categories.length === 0) return;
    setSubmitting(true);
    try {
      await startScan({
        projectId,
        targetId: target.id,
        endpointIds,
        profile: attackPlan.profileId,
        categories: attackPlan.categories,
        disabledTests: attackPlan.disabledTests,
      });
      void actions.refresh();
      notify("Scan started — running in the background", "success");
      navigate("/scans");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to start scan";
      notify(message, "error");
      setSubmitting(false);
    }
  }

  return (
    <div className="wizard-step">
      <div className="wizard-step__heading">
        <span className="wizard-step__number">5</span>
        <div>
          <h3 className="wizard-step__title">Start scan</h3>
          <p className="wizard-step__hint text-muted">
            Review the configuration and submit. The scan runs in the background.
          </p>
        </div>
      </div>

      <dl className="wizard-submit-summary">
        <div className="wizard-submit-summary__row">
          <dt>Target</dt>
          <dd>{target.url}</dd>
        </div>
        <div className="wizard-submit-summary__row">
          <dt>Profile</dt>
          <dd>{profileLabel}</dd>
        </div>
        <div className="wizard-submit-summary__row">
          <dt>Endpoints</dt>
          <dd>{endpointIds.length}</dd>
        </div>
        <div className="wizard-submit-summary__row">
          <dt>Categories</dt>
          <dd>{attackPlan.categories.length}</dd>
        </div>
        <div className="wizard-submit-summary__row">
          <dt>Estimated requests</dt>
          <dd>{estimatedRequests.toLocaleString()}</dd>
        </div>
        <div className="wizard-submit-summary__row">
          <dt>Estimated runtime</dt>
          <dd>{estimatedRuntime}</dd>
        </div>
      </dl>

      <div className="wizard-submit-actions">
        <Button
          variant="primary"
          onClick={() => void handleStartScan()}
          disabled={submitting || attackPlan.categories.length === 0}
        >
          {submitting ? "Starting…" : "Start Scan"}
        </Button>
        <p className="text-muted text-sm">
          You will be redirected to the Scans page immediately. Progress updates there while the
          job runs.
        </p>
      </div>
    </div>
  );
}
