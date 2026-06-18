import type { AttackPlanConfig } from "./attackProfiles";
import type { DiscoverySelection } from "./steps/DiscoveryStep";
import type { TargetFormState } from "./targetDescriptor";
import { isTargetFormValid } from "./wizardState";
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
    label: "Target",
    title: "Target & authentication",
    hint: "Enter the scan target URL and optional credentials",
  },
  {
    id: 3,
    label: "Discovery",
    title: "Discovery",
    hint: "Run discovery, fingerprint AI platforms, and select endpoints for attack planning",
  },
  {
    id: 4,
    label: "Attack Planning",
    title: "Attack planning",
    hint: "Review fingerprint-based attack suggestions and choose a profile",
  },
  {
    id: 5,
    label: "Scan Submission",
    title: "Scan submission",
    hint: "Review configuration and start the background scan job",
  },
  {
    id: 6,
    label: "Results",
    title: "Results",
    hint: "Review findings and export reports from SQLite",
  },
];

export type WizardDraft = {
  projectId: string;
  targetForm: TargetFormState;
  target: Target | null;
  discovery: DiscoverySelection;
  discoveryCompleted: boolean;
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
      return draft.target !== null && isTargetFormValid(draft.targetForm);
    case 3:
      return draft.discoveryCompleted && draft.discovery.selectedCount > 0;
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
    draft.discovery.selectedCount > 0 &&
    (draft.attackPlan?.categories.length ?? 0) > 0 &&
    draft.submittedScanId === null
  );
}

export function maxCompletableStep(draft: WizardDraft): WizardStepId {
  if (!draft.projectId.trim()) return 1;
  if (!isTargetFormValid(draft.targetForm) || !draft.target) return 2;
  if (!draft.discoveryCompleted || draft.discovery.selectedCount === 0) return 3;
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
      return isTargetFormValid(draft.targetForm);
    case 3:
      return draft.discoveryCompleted && draft.discovery.selectedCount > 0;
    case 4:
      return (draft.attackPlan?.categories.length ?? 0) > 0;
    case 5:
      return draft.submittedScanId !== null;
    default:
      return false;
  }
}
