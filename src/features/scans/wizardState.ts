import type { AttackPlanConfig, AttackCategoryId, AttackProfileId } from "./attackProfiles";
import type { TargetFormState } from "./targetDescriptor";
import {
  createInitialTargetForm,
  migrateTargetForm,
  targetFormFingerprint,
  targetFormFromDescriptor,
  targetFormMatchesDescriptor,
} from "./targetDescriptor";
import {
  createInitialTargetProfile,
  profileFromDto,
  type TargetProfileFormState,
  type VerificationConsoleEntryDto,
} from "./targetProfile";
import type { WizardStepId } from "./wizardSteps";
import type { Target } from "@/shared/types";

const STORAGE_KEY = "aisec:scan-wizard";
const STORAGE_VERSION = 3;

export type AttackPlanUiState = {
  profileId: AttackProfileId;
  customCategories: AttackCategoryId[];
  expandedCategory: AttackCategoryId | null;
  disabledTests: string[];
  plannerSummary: string | null;
  plannerMode: "deterministic" | "local_llm" | null;
  generatorMode: "static_pack" | "template_mutation" | "local_llm";
  generatorSummary: string | null;
  agentMode: boolean;
  maxAgentAttempts: number;
};

export type ScanWizardSession = {
  version: typeof STORAGE_VERSION;
  currentStep: WizardStepId;
  selectedProjectId: string;
  targetProfile: TargetProfileFormState;
  targetForm: TargetFormState;
  savedTargetId: string | null;
  savedTargetFingerprint: string | null;
  verificationConsole: VerificationConsoleEntryDto | null;
  attackPlanUi: AttackPlanUiState;
  attackPlan: AttackPlanConfig | null;
  submittedScanId: string | null;
};

export type ScanWizardStore = ScanWizardSession & {
  savedTarget: Target | null;
  profileVerified: boolean;
};

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
    plannerSummary: null,
    plannerMode: null,
    generatorMode: "static_pack",
    generatorSummary: null,
    agentMode: false,
    maxAgentAttempts: 5,
  };
}

export function createInitialSession(lockedProjectId = ""): ScanWizardSession {
  return {
    version: STORAGE_VERSION,
    currentStep: 1,
    selectedProjectId: lockedProjectId,
    targetProfile: createInitialTargetProfile(),
    targetForm: createInitialTargetForm(),
    savedTargetId: null,
    savedTargetFingerprint: null,
    verificationConsole: null,
    attackPlanUi: createInitialAttackPlanUi(),
    attackPlan: null,
    submittedScanId: null,
  };
}

export function buildWizardStore(session: ScanWizardSession, targets: Target[]): ScanWizardStore {
  const savedTarget = session.savedTargetId
    ? targets.find((target) => target.id === session.savedTargetId) ?? null
    : null;

  return {
    ...session,
    savedTarget,
    profileVerified: session.targetProfile.verification.verified,
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
      targetProfile: { ...createInitialTargetProfile(), ...parsed.targetProfile },
      targetForm: migrateTargetForm(parsed.targetForm ?? {}),
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
    // Ignore quota errors.
  }
}

export function clearWizardSession(): void {
  if (typeof window === "undefined") return;
  window.sessionStorage.removeItem(STORAGE_KEY);
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

export function createSessionForTargetScan(
  projectId: string,
  target: Target,
  descriptor: unknown,
  profile: unknown,
  step: WizardStepId = 3,
): ScanWizardSession {
  const targetForm = targetFormFromDescriptor(descriptor, target.url);
  const targetProfile =
    profile && typeof profile === "object"
      ? profileFromDto(profile as Parameters<typeof profileFromDto>[0])
      : createInitialTargetProfile();

  return {
    ...createInitialSession(projectId),
    selectedProjectId: projectId,
    currentStep: step,
    savedTargetId: target.id,
    savedTargetFingerprint: targetFormFingerprint(targetForm),
    targetForm,
    targetProfile,
  };
}

export function buildScanWizardUrl(projectId: string, targetId: string): string {
  const params = new URLSearchParams({ projectId, targetId });
  return `/scans/new?${params.toString()}`;
}

export async function fetchTargetFormForWizard(
  targetId: string,
  fallbackUrl = "",
): Promise<TargetFormState> {
  const dto = await loadTargetDtoForWizard(targetId);
  const url =
    fallbackUrl ||
    (typeof (dto.descriptor as { url?: string } | null)?.url === "string"
      ? (dto.descriptor as { url: string }).url
      : "");
  return targetFormFromDescriptor(dto.descriptor, url);
}

export async function loadTargetDtoForWizard(targetId: string) {
  const { getTarget, getTargetWizardDescriptor } = await import("@/shared/ipc/client");
  try {
    return await getTargetWizardDescriptor(targetId);
  } catch {
    return getTarget(targetId);
  }
}
