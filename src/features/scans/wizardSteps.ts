import type { AttackPlanConfig } from "./attackPlan";
import type { TargetFormState } from "./targetDescriptor";
import { validateTargetProfile, type TargetProfileFormState } from "./targetProfile";
import { isScanResultsReady } from "./wizardState";
import type { Target } from "@/shared/types";

export type WizardStepId = 1 | 2 | 3 | 4 | 5 | 6;

export type WizardStepDefinition = {
  id: WizardStepId;
  label: string;
  title: string;
  hint: string;
};

export const WIZARD_STEPS: WizardStepDefinition[] = [
  {
    id: 1,
    label: "Project",
    title: "Project",
    hint: "Choose the project this scan belongs to",
  },
  {
    id: 2,
    label: "AI Target Profile",
    title: "AI Target Profile",
    hint: "Select an AI platform and configure the request template",
  },
  {
    id: 3,
    label: "Authentication",
    title: "Authentication & verification",
    hint: "Configure credentials, then verify the AI API endpoint with Yazg",
  },
  {
    id: 4,
    label: "Attack Plan",
    title: "Attack Plan",
    hint: "Yazg analyzes your AI API Endpoint and builds an attack plan — review and adjust before running.",
  },
  {
    id: 5,
    label: "Attack",
    title: "Attack",
    hint: "Run the attack plan and monitor progress",
  },
  {
    id: 6,
    label: "Results",
    title: "Results",
    hint: "Review findings and export reports",
  },
];

export type WizardDraft = {
  projectId: string;
  target: Target | null;
  targetProfile: TargetProfileFormState;
  targetForm: TargetFormState;
  profileVerified: boolean;
  attackPlan: AttackPlanConfig | null;
  attackPlanGenerated: boolean;
  submittedScanId: string | null;
};

export function getWizardStep(id: WizardStepId): WizardStepDefinition {
  return WIZARD_STEPS.find((step) => step.id === id) ?? WIZARD_STEPS[0];
}

export function isStepComplete(step: WizardStepId, draft: WizardDraft): boolean {
  switch (step) {
    case 1:
      return draft.projectId.trim().length > 0;
    case 2:
      return draft.target !== null;
    case 3:
      return draft.profileVerified;
    case 4:
      return draft.attackPlanGenerated && (draft.attackPlan?.categories.length ?? 0) > 0;
    case 5:
      return draft.submittedScanId !== null;
    case 6:
      return draft.submittedScanId !== null;
    default:
      return false;
  }
}

export function canStartScan(draft: WizardDraft): boolean {
  return (
    draft.projectId.trim().length > 0 &&
    draft.target !== null &&
    draft.profileVerified &&
    draft.attackPlanGenerated &&
    (draft.attackPlan?.categories.length ?? 0) > 0 &&
    draft.submittedScanId === null
  );
}

export function maxCompletableStep(draft: WizardDraft): WizardStepId {
  if (!draft.projectId.trim()) return 1;
  if (!draft.target) return 2;
  if (!draft.profileVerified) return 3;
  if (!draft.attackPlanGenerated || (draft.attackPlan?.categories.length ?? 0) === 0) return 4;
  if (!draft.submittedScanId) return 5;
  return 6;
}

export type WizardNavigationOptions = {
  scanStatus?: string | null;
};

export function canNavigateToStep(
  target: WizardStepId,
  draft: WizardDraft,
  options?: WizardNavigationOptions,
): boolean {
  if (draft.submittedScanId) {
    if (target === 6) {
      return isScanResultsReady(options?.scanStatus);
    }
    return target === 5;
  }
  if (target === 1) return true;
  if (target === 6) return false;
  const max = maxCompletableStep(draft);
  if (target <= max) return true;
  return target === max + 1 && isStepComplete(max, draft);
}

export function canProceedFromStep(step: WizardStepId, draft: WizardDraft): boolean {
  switch (step) {
    case 1:
      return draft.projectId.trim().length > 0;
    case 2:
      return (
        draft.projectId.trim().length > 0 &&
        validateTargetProfile(draft.targetProfile) === null
      );
    case 3:
      return draft.profileVerified;
    case 4:
      return draft.attackPlanGenerated && (draft.attackPlan?.categories.length ?? 0) > 0;
    case 5:
      return draft.submittedScanId !== null;
    default:
      return false;
  }
}
