import type { AttackPlanConfig, AttackCategoryId, AttackProfileId } from "./attackProfiles";
import type { DiscoverySelection } from "./steps/DiscoveryStep";
import type { TargetFormState } from "./targetDescriptor";
import {
  createInitialTargetForm,
  migrateTargetForm,
  targetFormFingerprint,
  targetFormMatchesDescriptor,
  validateTargetStep,
} from "./targetDescriptor";
import type { WizardStepId } from "./wizardSteps";
import type { DiscoveryStatsDto } from "@/shared/ipc";
import type { Target } from "@/shared/types";

const STORAGE_KEY = "aisec:scan-wizard";
const STORAGE_VERSION = 2;

export type DiscoveryWizardState = {
  scanId: string | null;
  selectedEndpointIds: string[];
  completed: boolean;
  stats: DiscoveryStatsDto | null;
  manualMethod: string;
  manualPath: string;
};

export type AttackPlanUiState = {
  profileId: AttackProfileId;
  customCategories: AttackCategoryId[];
  expandedCategory: AttackCategoryId | null;
  disabledTests: string[];
};

export type ScanWizardSession = {
  version: typeof STORAGE_VERSION;
  currentStep: WizardStepId;
  selectedProjectId: string;
  targetForm: TargetFormState;
  savedTargetId: string | null;
  savedTargetFingerprint: string | null;
  discovery: DiscoveryWizardState;
  attackPlanUi: AttackPlanUiState;
  attackPlan: AttackPlanConfig | null;
  submittedScanId: string | null;
};

export type ScanWizardStore = ScanWizardSession & {
  savedTarget: Target | null;
  discoverySelection: DiscoverySelection;
};

export function createInitialDiscoveryState(): DiscoveryWizardState {
  return {
    scanId: null,
    selectedEndpointIds: [],
    completed: false,
    stats: null,
    manualMethod: "GET",
    manualPath: "",
  };
}

export function createInitialAttackPlanUi(): AttackPlanUiState {
  return {
    profileId: "standard",
    customCategories: [
      "prompt_injection",
      "system_prompt_extraction",
      "jailbreak",
      "rag_leakage",
      "memory_poisoning",
      "cross_user_leakage",
      "agent_goal_hijacking",
      "tool_abuse",
      "mcp_abuse",
    ],
    expandedCategory: null,
    disabledTests: [],
  };
}

export function createInitialSession(lockedProjectId = ""): ScanWizardSession {
  return {
    version: STORAGE_VERSION,
    currentStep: 1,
    selectedProjectId: lockedProjectId,
    targetForm: createInitialTargetForm(),
    savedTargetId: null,
    savedTargetFingerprint: null,
    discovery: createInitialDiscoveryState(),
    attackPlanUi: createInitialAttackPlanUi(),
    attackPlan: null,
    submittedScanId: null,
  };
}

function toDiscoverySelection(discovery: DiscoveryWizardState): DiscoverySelection {
  return {
    scanId: discovery.scanId,
    selectedCount: discovery.selectedEndpointIds.length,
    selectedEndpointIds: discovery.selectedEndpointIds,
  };
}

export function buildWizardStore(
  session: ScanWizardSession,
  targets: Target[],
): ScanWizardStore {
  const savedTarget = session.savedTargetId
    ? targets.find((target) => target.id === session.savedTargetId) ?? null
    : null;

  return {
    ...session,
    savedTarget,
    discoverySelection: toDiscoverySelection(session.discovery),
  };
}

export function loadWizardSession(lockedProjectId: string): ScanWizardSession {
  if (typeof window === "undefined") {
    return createInitialSession(lockedProjectId);
  }

  try {
    const raw = window.sessionStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return createInitialSession(lockedProjectId);
    }

    const parsed = JSON.parse(raw) as ScanWizardSession;
    if (parsed.version !== STORAGE_VERSION) {
      return createInitialSession(lockedProjectId);
    }

    return {
      ...createInitialSession(lockedProjectId),
      ...parsed,
      selectedProjectId: lockedProjectId || parsed.selectedProjectId,
      targetForm: migrateTargetForm(parsed.targetForm ?? {}),
      discovery: { ...createInitialDiscoveryState(), ...parsed.discovery },
      attackPlanUi: { ...createInitialAttackPlanUi(), ...parsed.attackPlanUi },
    };
  } catch {
    return createInitialSession(lockedProjectId);
  }
}

export function saveWizardSession(session: ScanWizardSession): void {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(session));
  } catch {
    // Ignore quota or private-mode errors; in-memory state still works.
  }
}

export function clearWizardSession(): void {
  if (typeof window === "undefined") return;
  window.sessionStorage.removeItem(STORAGE_KEY);
}

export function isTargetFormValid(form: TargetFormState): boolean {
  return validateTargetStep(form) === null;
}

export function shouldPersistTarget(
  form: TargetFormState,
  savedTarget: Target | null,
  savedFingerprint: string | null,
): boolean {
  if (!savedTarget) return true;
  const fingerprint = targetFormFingerprint(form);
  return !targetFormMatchesDescriptor(savedTarget, form, fingerprint, savedFingerprint);
}

export function resetSessionForNewScan(
  lockedProjectId: string,
  selectedProjectId: string,
): ScanWizardSession {
  return {
    ...createInitialSession(lockedProjectId),
    selectedProjectId: lockedProjectId || selectedProjectId,
  };
}
