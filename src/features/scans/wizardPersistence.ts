import type { AttackPlanConfig } from "./attackPlan";
import { attackPlanFromDto, type WizardAttackPlanDto } from "./attackPlan";
import type { AttackPlanUiState, ScanWizardSession } from "./wizardState";
import { createInitialAttackPlanUi } from "./wizardState";
import {
  createInitialTargetForm,
  migrateTargetForm,
  targetFormNeedsSecretHydration,
} from "./targetDescriptor";
import {
  createInitialTargetProfile,
  normalizeVerification,
} from "./targetProfile";
import type { WizardStepId } from "./wizardSteps";

export const WIZARD_DB_VERSION = 6;

export type WizardPersistedState = {
  version: typeof WIZARD_DB_VERSION;
  currentStep: WizardStepId;
  selectedProjectId: string;
  savedTargetId: string | null;
  savedTargetFingerprint: string | null;
  verificationConsole: ScanWizardSession["verificationConsole"];
  attackPlanUi: AttackPlanUiState;
  attackPlan: AttackPlanConfig | null;
  submittedScanId: string | null;
  targetProfile: ScanWizardSession["targetProfile"];
  targetForm: ScanWizardSession["targetForm"];
};

export function wizardStateToPersisted(session: ScanWizardSession): WizardPersistedState {
  return {
    version: WIZARD_DB_VERSION,
    currentStep: session.currentStep,
    selectedProjectId: session.selectedProjectId,
    savedTargetId: session.savedTargetId,
    savedTargetFingerprint: session.savedTargetFingerprint,
    verificationConsole: session.verificationConsole,
    attackPlanUi: session.attackPlanUi,
    attackPlan: session.attackPlan,
    submittedScanId: session.submittedScanId,
    targetProfile: session.targetProfile,
    targetForm: session.targetForm,
  };
}

export function sessionFromPersistedWizard(
  persisted: WizardPersistedState,
  draftScanId: string,
  lockedProjectId = "",
): ScanWizardSession {
  return {
    version: WIZARD_DB_VERSION,
    draftScanId,
    currentStep: persisted.currentStep,
    selectedProjectId: lockedProjectId || persisted.selectedProjectId,
    targetProfile: {
      ...createInitialTargetProfile(),
      ...persisted.targetProfile,
      verification: normalizeVerification(persisted.targetProfile?.verification),
    },
    targetForm: migrateTargetForm(persisted.targetForm ?? createInitialTargetForm()),
    savedTargetId: persisted.savedTargetId,
    savedTargetFingerprint: persisted.savedTargetFingerprint,
    verificationConsole: persisted.verificationConsole,
    attackPlanUi: { ...createInitialAttackPlanUi(), ...persisted.attackPlanUi },
    attackPlan: persisted.attackPlan,
    submittedScanId: persisted.submittedScanId,
  };
}

export function parsePersistedWizard(raw: unknown): WizardPersistedState | null {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Partial<WizardPersistedState>;
  if (value.version !== WIZARD_DB_VERSION) return null;
  if (typeof value.currentStep !== "number") return null;
  return {
    version: WIZARD_DB_VERSION,
    currentStep: value.currentStep as WizardStepId,
    selectedProjectId: typeof value.selectedProjectId === "string" ? value.selectedProjectId : "",
    savedTargetId: typeof value.savedTargetId === "string" ? value.savedTargetId : null,
    savedTargetFingerprint:
      typeof value.savedTargetFingerprint === "string" ? value.savedTargetFingerprint : null,
    verificationConsole: value.verificationConsole ?? null,
    attackPlanUi: { ...createInitialAttackPlanUi(), ...(value.attackPlanUi ?? {}) },
    attackPlan: value.attackPlan ?? null,
    submittedScanId: typeof value.submittedScanId === "string" ? value.submittedScanId : null,
    targetProfile: {
      ...createInitialTargetProfile(),
      ...(value.targetProfile ?? {}),
      verification: normalizeVerification(value.targetProfile?.verification),
    },
    targetForm: migrateTargetForm(value.targetForm ?? createInitialTargetForm()),
  };
}

export function attackPlanFromPersistedDto(dto: WizardAttackPlanDto): AttackPlanConfig {
  return attackPlanFromDto(dto);
}

/** Prefer the session copy that still has auth secrets filled in. */
export function mergeWizardSessions(
  local: ScanWizardSession,
  remote: ScanWizardSession,
): ScanWizardSession {
  const localHasSecrets =
    local.targetForm.authKind !== "none" && !targetFormNeedsSecretHydration(local.targetForm);
  const remoteHasSecrets =
    remote.targetForm.authKind !== "none" && !targetFormNeedsSecretHydration(remote.targetForm);

  let targetForm = remote.targetForm;
  if (localHasSecrets && !remoteHasSecrets) {
    targetForm = local.targetForm;
  } else if (local.targetForm.authKind !== "none" && remote.targetForm.authKind === "none") {
    targetForm = local.targetForm;
  } else if (local.targetForm.authKind !== "none") {
    targetForm = { ...remote.targetForm, ...local.targetForm };
  }

  const targetProfile =
    local.targetProfile.verification.verified && !remote.targetProfile.verification.verified
      ? local.targetProfile
      : remote.targetProfile;

  return {
    ...remote,
    currentStep: Math.max(local.currentStep, remote.currentStep) as WizardStepId,
    targetForm,
    targetProfile,
    verificationConsole: local.verificationConsole ?? remote.verificationConsole,
    attackPlan: remote.attackPlan ?? local.attackPlan,
    attackPlanUi: remote.attackPlan ? remote.attackPlanUi : local.attackPlanUi,
    submittedScanId: remote.submittedScanId ?? local.submittedScanId,
  };
}
