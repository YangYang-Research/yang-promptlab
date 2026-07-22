import type { AttackPlanConfig } from "./attackPlan";
import type { AttackCategoryId, AttackProfileId } from "./attackProfiles";
import type { TargetFormState } from "./targetDescriptor";
import {
  createInitialTargetForm,
  migrateTargetForm,
  syncAuthFormFromProfile,
  inferFreshAuthFormFromProfile,
  targetFormFingerprint,
  targetFormFromDescriptor,
  targetFormMatchesDescriptor,
  targetFormNeedsSecretHydration,
} from "./targetDescriptor";
import {
  createEmptyVerification,
  createInitialTargetProfile,
  normalizeVerification,
  profileFromDto,
  fullProfileUrl,
  type TargetProfileFormState,
} from "./targetProfile";
import type { VerificationLogLine } from "./verificationLog";
import type { WizardStepId } from "./wizardSteps";
import type { ScanRun, Target } from "@/shared/types";

const STORAGE_KEY = "promptlab:scan-wizard";
const STORAGE_VERSION = 7;

export type PlannerSource = "ai_runtime" | "target_profile";

/** Origin of the current attack plan — imported plans survive step-3 re-verify. */
export type AttackPlanSource = "imported" | "generated";

export type AttackPlanUiState = {
  profileId: AttackProfileId;
  customCategories: AttackCategoryId[];
  expandedCategory: AttackCategoryId | null;
  disabledTests: string[];
  disabledGraphNodes: AttackCategoryId[];
  /** Set when a planner run completes; used for the summary badge baseline. */
  plannerSource: PlannerSource | null;
  suggestedPlanKey: string | null;
};

export type ScanWizardSession = {
  version: typeof STORAGE_VERSION;
  draftScanId: string | null;
  currentStep: WizardStepId;
  selectedProjectId: string;
  targetProfile: TargetProfileFormState;
  targetForm: TargetFormState;
  savedTargetId: string | null;
  savedTargetFingerprint: string | null;
  verificationLog: VerificationLogLine[];
  attackPlanUi: AttackPlanUiState;
  attackPlan: AttackPlanConfig | null;
  /** null when no plan; "imported" skips wipe on verify success. */
  attackPlanSource: AttackPlanSource | null;
  /**
   * When true, wizard auto-advances steps 1→3 (countdown), auto-verifies with
   * retries, then lands on step 4 for plan review.
   */
  importAutoAdvance: boolean;
  submittedScanId: string | null;
};

export type ScanWizardStore = ScanWizardSession & {
  savedTarget: Target | null;
  profileVerified: boolean;
};

export function attackPlanUiFromPlan(plan: AttackPlanConfig): AttackPlanUiState {
  return {
    profileId: plan.profileId,
    customCategories: plan.customCategories,
    expandedCategory: null,
    disabledTests: plan.disabledTests,
    disabledGraphNodes: plan.disabledGraphNodes,
    plannerSource: null,
    suggestedPlanKey: null,
  };
}

export function createInitialAttackPlanUi(): AttackPlanUiState {
  return {
    profileId: "standard",
    customCategories: [],
    expandedCategory: null,
    disabledTests: [],
    disabledGraphNodes: [],
    plannerSource: null,
    suggestedPlanKey: null,
  };
}

export function createInitialSession(lockedProjectId = ""): ScanWizardSession {
  return {
    version: STORAGE_VERSION,
    draftScanId: null,
    currentStep: 1,
    selectedProjectId: lockedProjectId,
    targetProfile: createInitialTargetProfile(),
    targetForm: createInitialTargetForm(),
    savedTargetId: null,
    savedTargetFingerprint: null,
    verificationLog: [],
    attackPlanUi: createInitialAttackPlanUi(),
    attackPlan: null,
    attackPlanSource: null,
    importAutoAdvance: false,
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

export function peekWizardSession(): ScanWizardSession | null {
  if (typeof window === "undefined") return null;

  try {
    const raw = window.sessionStorage.getItem(STORAGE_KEY);
    if (!raw) return null;

    const parsed = JSON.parse(raw) as ScanWizardSession;
    if (parsed.version !== STORAGE_VERSION) return null;

    return {
      ...createInitialSession(),
      ...parsed,
      targetProfile: {
        ...createInitialTargetProfile(),
        ...parsed.targetProfile,
        verification: normalizeVerification(parsed.targetProfile?.verification),
      },
      targetForm: migrateTargetForm(parsed.targetForm ?? {}),
      attackPlanUi: { ...createInitialAttackPlanUi(), ...parsed.attackPlanUi },
    };
  } catch {
    return null;
  }
}

export function wizardResumeInputFromSession(session: ScanWizardSession): {
  savedTargetId: string | null;
  selectedProjectId: string;
  currentStep: WizardStepId;
  profileVerified: boolean;
  attackPlanGenerated: boolean;
  submittedScanId: string | null;
} {
  return {
    savedTargetId: session.savedTargetId,
    selectedProjectId: session.selectedProjectId,
    currentStep: session.currentStep,
    profileVerified: session.targetProfile.verification.verified,
    attackPlanGenerated: session.attackPlan !== null,
    submittedScanId: session.submittedScanId,
  };
}

export function loadWizardSession(lockedProjectId: string): ScanWizardSession {
  const peeked = peekWizardSession();
  if (peeked) {
    return {
      ...peeked,
      selectedProjectId: lockedProjectId || peeked.selectedProjectId,
    };
  }

  return createInitialSession(lockedProjectId);
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
    draftScanId: null,
    selectedProjectId: projectId,
    currentStep: step,
    savedTargetId: target.id,
    savedTargetFingerprint: targetFormFingerprint(targetForm),
    targetForm,
    targetProfile,
  };
}

export function parseWizardEntryStep(raw: string): WizardStepId | null {
  const parsed = Number.parseInt(raw, 10);
  if (parsed >= 1 && parsed <= 6) {
    return parsed as WizardStepId;
  }
  return null;
}

/** True when the URL requests a brand-new wizard (not resume / target / step deep link). */
export function isFreshWizardEntry(params: {
  scanId?: string;
  targetId?: string;
  step?: string;
}): boolean {
  const scanId = params.scanId?.trim() ?? "";
  const targetId = params.targetId?.trim() ?? "";
  const step = parseWizardEntryStep(params.step?.trim() ?? "");
  return !scanId && !targetId && step === null;
}

/** Apply explicit wizard entry intent from URL deep links (new scan, retry, resume). */
export function applyWizardEntryStep(
  session: ScanWizardSession,
  step: WizardStepId | null,
): ScanWizardSession {
  if (!step) return session;

  if (step === 2) {
    // New-target flow: blank step 2.
    if (!session.savedTargetId) {
      const projectId = session.selectedProjectId;
      return {
        ...createInitialSession(projectId),
        currentStep: 2,
        selectedProjectId: projectId,
      };
    }
    // Existing target (e.g. New Scan from target details): keep target, clear prior run state.
    return {
      ...session,
      currentStep: 2,
      draftScanId: null,
      submittedScanId: null,
      attackPlan: null,
      attackPlanSource: null,
      attackPlanUi: createInitialAttackPlanUi(),
      importAutoAdvance: false,
    };
  }

  if (step === 4) {
    return {
      ...session,
      currentStep: 4,
      submittedScanId: null,
    };
  }

  if (step === 5) {
    return {
      ...session,
      currentStep: 5,
      submittedScanId: session.submittedScanId ?? session.draftScanId,
    };
  }

  return { ...session, currentStep: step };
}

export function buildScanWizardUrl(
  projectId: string,
  targetId?: string,
  options?: { step?: WizardStepId; scanId?: string },
): string {
  const params = new URLSearchParams({ projectId });
  if (targetId) {
    params.set("targetId", targetId);
  }
  if (options?.scanId) {
    params.set("scanId", options.scanId);
  }
  if (options?.step) {
    params.set("step", String(options.step));
  }
  return `/scans/new?${params.toString()}`;
}

export function isLiveScanStatus(status: string): boolean {
  return status === "running" || status === "paused" || status === "pending";
}

export function isRetryableScanStatus(status: string): boolean {
  return status === "failed" || status === "cancelled" || status === "stopped";
}

export function isScanResultsReady(status: string | null | undefined): boolean {
  return status === "completed";
}

export function buildScanProgressUrl(
  projectId: string,
  scanId: string,
  targetId?: string | null,
): string {
  return buildScanWizardUrl(projectId, targetId ?? undefined, { scanId, step: 5 });
}

export function buildScanRetryUrl(
  projectId: string,
  scanId: string,
  targetId?: string | null,
): string {
  return buildScanWizardUrl(projectId, targetId ?? undefined, { scanId, step: 4 });
}

/** Open wizard submit step and auto-restart the attack once hydrated. */
export function buildScanStartAttackUrl(
  projectId: string,
  scanId: string,
  targetId?: string | null,
): string {
  return `${buildScanRetryUrl(projectId, scanId, targetId)}&autoStart=1`;
}

export function resolveScanNavigationStatus(
  storeStatus: string,
  liveStatus?: string | null,
): string {
  if (!isLiveScanStatus(storeStatus)) {
    return storeStatus;
  }
  return liveStatus ?? storeStatus;
}

export function resolveScanOpenPath(
  scan: Pick<ScanRun, "id" | "projectId" | "targetId" | "status">,
  liveStatus?: string | null,
): string {
  const status = resolveScanNavigationStatus(scan.status, liveStatus);
  if (isLiveScanStatus(status)) {
    return buildScanProgressUrl(scan.projectId, scan.id, scan.targetId);
  }
  if (isRetryableScanStatus(status)) {
    return buildScanRetryUrl(scan.projectId, scan.id, scan.targetId);
  }
  if (scan.status === "draft") {
    return buildScanWizardUrl(scan.projectId, scan.targetId ?? undefined, { scanId: scan.id });
  }
  return `/scans/${scan.id}`;
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

export async function prepareAuthFormForStep3(
  profile: TargetProfileFormState,
  current: TargetFormState,
  targetId: string | null,
  options?: { reinferFromProfile?: boolean },
): Promise<TargetFormState> {
  const profileUrl = fullProfileUrl(profile);
  const base = options?.reinferFromProfile
    ? inferFreshAuthFormFromProfile(profile)
    : { ...current, url: profileUrl };
  let form = syncAuthFormFromProfile(profile, base);

  if (targetId && targetFormNeedsSecretHydration(form)) {
    try {
      const fromWizard = await fetchTargetFormForWizard(targetId, profileUrl);
      form = syncAuthFormFromProfile(profile, {
        ...form,
        ...fromWizard,
        url: profileUrl,
        authKind: form.authKind !== "none" ? form.authKind : fromWizard.authKind,
      });
    } catch {
      // Fall back to profile header hydration only.
    }
  }

  return syncAuthFormFromProfile(profile, form);
}
