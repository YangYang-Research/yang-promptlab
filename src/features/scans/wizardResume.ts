import { getScan } from "@/shared/ipc";
import type { ScanWizardLoadDto } from "@/shared/ipc/scanWizard";
import { getTargetProfile } from "@/shared/ipc/targetProfile";
import type { Target } from "@/shared/types";

import {
  attackPlanFromExecutionPlaybook,
  normalizeAttackPlan,
  type AttackPlanConfig,
} from "./attackPlan";
import { targetFormFingerprint } from "./targetDescriptor";
import { profileFromDto } from "./targetProfile";
import {
  mergeWizardSessions,
  parsePersistedWizard,
  sessionFromPersistedWizard,
} from "./wizardPersistence";
import {
  applyWizardEntryStep,
  fetchTargetFormForWizard,
  type ScanWizardSession,
} from "./wizardState";
import type { WizardStepId } from "./wizardSteps";

export function sessionReadyForSubmitStep(
  session: ScanWizardSession,
  savedTarget: Target | null,
): boolean {
  const hasPlan = Boolean(session.attackPlan && session.attackPlan.categories.length > 0);
  const hasProject = Boolean(session.selectedProjectId);
  const canMonitorLiveScan = Boolean(session.submittedScanId && hasPlan && hasProject);

  if (canMonitorLiveScan && !savedTarget && session.savedTargetId) {
    return true;
  }

  return Boolean(
    hasProject &&
      savedTarget &&
      hasPlan &&
      (session.targetProfile.verification.verified || session.submittedScanId),
  );
}

/** Step-specific readiness for wizard deep links — avoids false positives from submittedScanId. */
export function sessionReadyForWizardEntry(
  session: ScanWizardSession,
  savedTarget: Target | null,
  entryStep: WizardStepId | null,
): boolean {
  const hasTarget = Boolean(savedTarget || session.savedTargetId);

  if (entryStep === 4) {
    return Boolean(
      session.selectedProjectId &&
        hasTarget &&
        session.targetProfile.verification.verified,
    );
  }

  if (entryStep === 5) {
    return sessionReadyForSubmitStep(session, savedTarget);
  }

  if (entryStep === 3) {
    return Boolean(session.selectedProjectId && hasTarget);
  }

  return Boolean(session.draftScanId);
}

export async function hydrateWizardSessionForScanResume(
  session: ScanWizardSession,
  loaded: ScanWizardLoadDto,
  options: {
    lockedProjectId: string;
    lockedTargetId: string;
    entryStep: WizardStepId | null;
  },
): Promise<ScanWizardSession> {
  const { lockedProjectId, lockedTargetId, entryStep } = options;
  const scan = loaded.scan;
  const projectId = lockedProjectId || scan.project_id;
  const targetId = lockedTargetId || scan.target_id || session.savedTargetId;

  let next: ScanWizardSession = {
    ...session,
    draftScanId: scan.id,
    selectedProjectId: projectId,
    savedTargetId: targetId ?? session.savedTargetId,
  };

  const persisted = parsePersistedWizard(loaded.wizard);
  if (persisted) {
    const remote = sessionFromPersistedWizard(persisted, scan.id, lockedProjectId);
    next =
      session.draftScanId === scan.id ? mergeWizardSessions(session, remote) : remote;
    next = {
      ...next,
      draftScanId: scan.id,
      selectedProjectId: projectId,
      savedTargetId: targetId ?? next.savedTargetId,
    };
  }

  if (entryStep) {
    next = applyWizardEntryStep(next, entryStep);
  }

  if (!next.attackPlan?.categories.length) {
    const detail = await getScan(scan.id);
    const rebuilt = attackPlanFromExecutionPlaybook(detail.playbook);
    if (rebuilt) {
      next = { ...next, attackPlan: rebuilt, attackPlanSource: "generated" };
    }
  } else if (next.attackPlan) {
    next = {
      ...next,
      attackPlan: normalizeAttackPlan(next.attackPlan),
      attackPlanSource: next.attackPlanSource ?? "generated",
    };
  }

  if (targetId && (entryStep === 4 || !next.targetProfile.verification.verified)) {
    try {
      const dto = await getTargetProfile(targetId);
      const profile = profileFromDto(dto);
      next = { ...next, targetProfile: profile };
    } catch {
      // Keep the persisted profile when target lookup fails.
    }
  }

  if (targetId) {
    try {
      const targetForm = await fetchTargetFormForWizard(targetId);
      next = {
        ...next,
        targetForm,
        savedTargetFingerprint: targetFormFingerprint(targetForm),
      };
    } catch {
      // Ignore descriptor hydration failures — SubmitStep can still render.
    }
  }

  // Draft = still in wizard review. Never treat it as submitted (clears polluted
  // local/remote wizard state that used to set submittedScanId from draftScanId).
  if (scan.status === "draft") {
    next = { ...next, submittedScanId: null };
  } else if (
    ["running", "paused", "pending", "completed", "failed", "cancelled", "stopped"].includes(
      scan.status,
    )
  ) {
    next = { ...next, submittedScanId: scan.id };
  }

  return next;
}

export function attackPlanCategoriesFromSession(
  session: ScanWizardSession,
): AttackPlanConfig["categories"] {
  return session.attackPlan?.categories ?? [];
}
