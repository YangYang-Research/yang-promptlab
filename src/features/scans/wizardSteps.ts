import type { AttackPlanConfig } from "./attackProfiles";
import type { TargetFormState } from "./targetDescriptor";
import type { TargetProfileFormState } from "./targetProfile";
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
    hint: "Configure credentials and verify the target responds to a real AI request",
  },
  {
    id: 4,
    label: "Attack Planning",
    title: "Attack planning",
    hint: "Review capability-based attack suggestions and choose a profile",
  },
  {
    id: 5,
    label: "Scan",
    title: "Scan submission",
    hint: "Review configuration and start the background scan job",
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
      return (draft.attackPlan?.categories.length ?? 0) > 0;
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
    (draft.attackPlan?.categories.length ?? 0) > 0 &&
    draft.submittedScanId === null
  );
}

export function maxCompletableStep(draft: WizardDraft): WizardStepId {
  if (!draft.projectId.trim()) return 1;
  if (!draft.target) return 2;
  if (!draft.profileVerified) return 3;
  if ((draft.attackPlan?.categories.length ?? 0) === 0) return 4;
  if (!draft.submittedScanId) return 5;
  return 6;
}

export function canNavigateToStep(target: WizardStepId, draft: WizardDraft): boolean {
  if (draft.submittedScanId) {
    return target === 5 || target === 6;
  }
  if (target === 1) return true;
  if (target === 6) return draft.submittedScanId !== null;
  const max = maxCompletableStep(draft);
  if (target <= max) return true;
  return target === max + 1 && isStepComplete(max, draft);
}

export function canProceedFromStep(step: WizardStepId, draft: WizardDraft): boolean {
  switch (step) {
    case 1:
      return draft.projectId.trim().length > 0;
    case 2:
      return draft.target !== null;
    case 3:
      return draft.profileVerified;
    case 4:
      return (draft.attackPlan?.categories.length ?? 0) > 0;
    case 5:
      return draft.submittedScanId !== null;
    default:
      return false;
  }
}
