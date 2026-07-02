import type { AttackPlanConfig } from "./attackPlan";
import { attackPlanFromDto, normalizeAttackPlan, type WizardAttackPlanDto } from "./attackPlan";
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
import type { VerificationLogLine } from "./verificationLog";
import type { VerificationConsoleEntryDto } from "./targetProfile";
import { migrateLegacyVerificationConsole } from "./verificationLog";
import type { WizardStepId } from "./wizardSteps";

export const WIZARD_DB_VERSION = 7;

function migrateVerificationLog(
  log: VerificationLogLine[] | unknown[] | undefined,
  legacyConsole: VerificationConsoleEntryDto | null | undefined,
): VerificationLogLine[] {
  if (Array.isArray(log) && log.length > 0) {
    const first = log[0];
    if (
      first &&
      typeof first === "object" &&
      "message" in first &&
      typeof (first as VerificationLogLine).message === "string" &&
      !("console" in first)
    ) {
      return log as VerificationLogLine[];
    }

    return log.flatMap((item) => {
      if (item && typeof item === "object" && "console" in item) {
        const console = (item as { console?: VerificationConsoleEntryDto }).console;
        return console ? migrateLegacyVerificationConsole(console) : [];
      }
      return [];
    });
  }
  if (legacyConsole) {
    return migrateLegacyVerificationConsole(legacyConsole);
  }
  return [];
}

export type WizardPersistedState = {
  version: typeof WIZARD_DB_VERSION;
  currentStep: WizardStepId;
  selectedProjectId: string;
  savedTargetId: string | null;
  savedTargetFingerprint: string | null;
  verificationLog: ScanWizardSession["verificationLog"];
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
    verificationLog: session.verificationLog,
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
    verificationLog: migrateVerificationLog(
      persisted.verificationLog,
      (persisted as { verificationConsole?: VerificationConsoleEntryDto | null }).verificationConsole,
    ),
    attackPlanUi: { ...createInitialAttackPlanUi(), ...persisted.attackPlanUi },
    attackPlan: persisted.attackPlan ? normalizeAttackPlan(persisted.attackPlan) : null,
    submittedScanId: persisted.submittedScanId,
  };
}

export function parsePersistedWizard(raw: unknown): WizardPersistedState | null {
  if (!raw || typeof raw !== "object") return null;
  const value = raw as Partial<WizardPersistedState> & {
    verificationConsole?: VerificationConsoleEntryDto | null;
  };
  if (value.version !== WIZARD_DB_VERSION && value.version !== 6) return null;
  if (typeof value.currentStep !== "number") return null;
  return {
    version: WIZARD_DB_VERSION,
    currentStep: value.currentStep as WizardStepId,
    selectedProjectId: typeof value.selectedProjectId === "string" ? value.selectedProjectId : "",
    savedTargetId: typeof value.savedTargetId === "string" ? value.savedTargetId : null,
    savedTargetFingerprint:
      typeof value.savedTargetFingerprint === "string" ? value.savedTargetFingerprint : null,
    verificationLog: migrateVerificationLog(
      value.verificationLog as VerificationLogLine[] | undefined,
      value.verificationConsole as VerificationConsoleEntryDto | null | undefined,
    ),
    attackPlanUi: { ...createInitialAttackPlanUi(), ...(value.attackPlanUi ?? {}) },
    attackPlan: value.attackPlan ? normalizeAttackPlan(value.attackPlan as AttackPlanConfig) : null,
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
    verificationLog:
      local.verificationLog.length > 0 ? local.verificationLog : remote.verificationLog,
    attackPlan:
      remote.attackPlan || local.attackPlan
        ? normalizeAttackPlan((remote.attackPlan ?? local.attackPlan)!)
        : null,
    attackPlanUi: remote.attackPlan ? remote.attackPlanUi : local.attackPlanUi,
    submittedScanId: remote.submittedScanId ?? local.submittedScanId,
  };
}
