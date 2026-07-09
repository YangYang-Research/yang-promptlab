import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { mapProjects, mapTargets } from "@/app/store/mappers";
import { Button, Card, PageHeader, Badge } from "@/shared/components";
import { getProject, getScanStatus, pauseScan, resumeScan, startScan, updateTargetDescriptor } from "@/shared/ipc";
import { createWizardScan, loadWizardScan, saveWizardScan } from "@/shared/ipc/scanWizard";
import { generateAttackPlanForTarget } from "@/shared/ipc/attackPlanner";
import { getTargetProfile, saveTargetProfile } from "@/shared/ipc/targetProfile";
import { toAppError } from "@/shared/errors";
import { useToast } from "@/shared/notifications";
import type { Project, Target } from "@/shared/types";

import { ImportApiModal } from "./components/ImportApiModal";
import { ReportExportDropdown } from "./components/ReportExportDropdown";
import { ReviewAttackPlanStep } from "./steps/ReviewAttackPlanStep";
import { AttackPlanPlanningState } from "./steps/AttackPlanPlanningState";
import { AuthVerificationStep } from "./steps/AuthVerificationStep";
import { ProjectStep } from "./steps/ProjectStep";
import { ResultsStep } from "./steps/ResultsStep";
import { SubmitStep } from "./steps/SubmitStep";
import { TargetProfileStep } from "./steps/TargetProfileStep";
import { mergeScanStatus, useScanStatuses } from "./useScanStatuses";
import {
  deriveTargetNameFromProfile,
  createEmptyVerification,
  fullProfileUrl,
  profileFromDto,
  profileToPayload,
  validateTargetProfile,
  verificationBadgeFromDb,
  type VerificationResultForm,
} from "./targetProfile";
import {
  buildTargetDescriptor,
  targetFormFingerprint,
  targetFormFromDescriptor,
  targetFormNeedsSecretHydration,
  validateTargetStep,
  type TargetFormState,
} from "./targetDescriptor";
import { WizardStepper } from "./WizardStepper";
import { attackPlanFromDto, attackPlanUiBaselineFromPlan, normalizeAttackPlan, payloadStrategyToDto } from "./attackPlan";
import {
  buildWizardStore,
  clearWizardSession,
  createInitialSession,
  createSessionForTargetScan,
  applyWizardEntryStep,
  parseWizardEntryStep,
  isFreshWizardEntry,
  createInitialAttackPlanUi,
  fetchTargetFormForWizard,
  loadTargetDtoForWizard,
  loadWizardSession,
  peekWizardSession,
  prepareAuthFormForStep3,
  saveWizardSession,
  type ScanWizardSession,
} from "./wizardState";
import {
  canNavigateToStep,
  canProceedFromStep,
  canStartScan,
  getWizardStep,
  type WizardDraft,
  type WizardStepId,
} from "./wizardSteps";
import {
  mergeWizardSessions,
  wizardStateToPersisted,
} from "./wizardPersistence";
import {
  hydrateWizardSessionForScanResume,
  sessionReadyForSubmitStep,
  sessionReadyForWizardEntry,
} from "./wizardResume";
import {
  resolveOrCreateDraftScanId,
  storeDraftScanId,
} from "./wizardDraftScan";

function withNormalizedAttackPlan(session: ScanWizardSession): ScanWizardSession {
  if (!session.attackPlan) return session;
  return { ...session, attackPlan: normalizeAttackPlan(session.attackPlan) };
}

export function ScanWizardPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const lockedProjectId = searchParams.get("projectId")?.trim() ?? "";
  const lockedTargetId = searchParams.get("targetId")?.trim() ?? "";
  const requestedStep = searchParams.get("step")?.trim() ?? "";
  const lockedScanId = searchParams.get("scanId")?.trim() ?? "";
  const entryStep = parseWizardEntryStep(requestedStep);
  const isFreshWizard = isFreshWizardEntry({
    scanId: lockedScanId,
    targetId: lockedTargetId,
    step: requestedStep,
  });
  const { projects, targets, scans, findings, loading, error, dispatch, actions } = useAppStore();
  const { notify } = useToast();
  const deepLinkApplied = useRef(false);
  const deepLinkTargetId = useRef<string | null>(null);

  const [session, setSession] = useState<ScanWizardSession>(() => {
    if (isFreshWizard) {
      clearWizardSession();
      return createInitialSession(lockedProjectId);
    }
    return withNormalizedAttackPlan(loadWizardSession(lockedProjectId));
  });
  const [resolvedProject, setResolvedProject] = useState<Project | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [profileStepError, setProfileStepError] = useState<string | null>(null);
  const [verificationError, setVerificationError] = useState<string | null>(null);
  const [scanSubmitError, setScanSubmitError] = useState<string | null>(null);
  const [persistingTarget, setPersistingTarget] = useState(false);
  const [startingScan, setStartingScan] = useState(false);
  const [plannerGenerating, setPlannerGenerating] = useState(false);
  const [plannerReplanning, setPlannerReplanning] = useState(false);
  const [plannerError, setPlannerError] = useState<string | null>(null);
  const [planAdjusting, setPlanAdjusting] = useState(false);
  const [dbVerification, setDbVerification] = useState<VerificationResultForm | null>(null);
  const [dbVerificationLoading, setDbVerificationLoading] = useState(false);
  const [importApiOpen, setImportApiOpen] = useState(false);
  const [scanControlPending, setScanControlPending] = useState(false);
  const [wizardResumeLoading, setWizardResumeLoading] = useState(() =>
    Boolean(lockedScanId && (entryStep === 4 || entryStep === 5)),
  );
  const plannerRunRef = useRef<string | null>(null);
  const startingScanRef = useRef(false);
  const wizardDbBootstrap = useRef(false);
  const sessionRef = useRef(session);
  sessionRef.current = session;
  const wizardSaveTimerRef = useRef<number | null>(null);
  const authHydratedKeyRef = useRef<string | null>(null);
  const freshWizardKeyRef = useRef<string | null>(null);
  const wizardResumeHydratedRef = useRef<string | null>(null);
  const newTargetEntryKeyRef = useRef<string | null>(null);

  function appendWizardUrlParams(params: URLSearchParams) {
    if (lockedTargetId) params.set("targetId", lockedTargetId);
    if (entryStep) params.set("step", String(entryStep));
  }

  const store = useMemo(
    () => buildWizardStore(session, targets),
    [session, targets],
  );

  const storeProject = lockedProjectId
    ? projects.find((project) => project.id === lockedProjectId)
    : null;
  const lockedProject = storeProject ?? resolvedProject;
  const activeProjectId = lockedProjectId || session.selectedProjectId;

  const draft: WizardDraft = useMemo(
    () => ({
      projectId: activeProjectId,
      targetForm: session.targetForm,
      target: store.savedTarget,
      targetProfile: session.targetProfile,
      profileVerified: store.profileVerified,
      attackPlan: session.attackPlan,
      attackPlanGenerated: session.attackPlan !== null,
      submittedScanId: session.submittedScanId,
    }),
    [
      activeProjectId,
      session.targetForm,
      store.savedTarget,
      session.targetProfile,
      store.profileVerified,
      session.attackPlan,
      session.submittedScanId,
    ],
  );

  const updateSession = useCallback((patch: Partial<ScanWizardSession>) => {
    setSession((prev) => {
      const normalizedPatch =
        patch.attackPlan !== undefined
          ? { ...patch, attackPlan: patch.attackPlan ? normalizeAttackPlan(patch.attackPlan) : null }
          : patch;
      const next = withNormalizedAttackPlan({ ...prev, ...normalizedPatch });
      saveWizardSession(next);
      return next;
    });
  }, []);

  const applyDraftScanId = useCallback(
    (scanId: string, projectId: string, patch: Partial<ScanWizardSession> = {}) => {
      storeDraftScanId(projectId, scanId);
      setSession((prev) => {
        const next = {
          ...prev,
          ...patch,
          draftScanId: scanId,
          selectedProjectId: projectId,
        };
        saveWizardSession(next);
        return next;
      });
    },
    [],
  );

  useEffect(() => {
    if (!isFreshWizard) return;

    const freshKey = `${lockedProjectId}|${lockedTargetId}|${requestedStep}|${lockedScanId}`;
    if (freshWizardKeyRef.current === freshKey) return;
    freshWizardKeyRef.current = freshKey;

    wizardDbBootstrap.current = false;
    deepLinkApplied.current = false;
    authHydratedKeyRef.current = null;
    plannerRunRef.current = null;
    wizardResumeHydratedRef.current = null;
    clearWizardSession();
    setPlannerError(null);
    setPlannerGenerating(false);
    setProfileStepError(null);
    setVerificationError(null);
    setScanSubmitError(null);
    setSession(createInitialSession(lockedProjectId));
  }, [isFreshWizard, lockedProjectId, lockedTargetId, requestedStep, lockedScanId]);

  useEffect(() => {
    if (wizardDbBootstrap.current) return;

    async function ensureDraftScan() {
      if (lockedScanId) {
        const savedTarget =
          session.savedTargetId
            ? targets.find((target) => target.id === session.savedTargetId) ?? null
            : null;
        const readyForEntry = sessionReadyForWizardEntry(session, savedTarget, entryStep);
        const resumeKey = `${lockedScanId}:${entryStep ?? ""}`;

        if (session.draftScanId === lockedScanId && readyForEntry) {
          if (entryStep) {
            setSession((prev) => {
              const next = applyWizardEntryStep(prev, entryStep);
              saveWizardSession(next);
              return next;
            });
          }
          wizardDbBootstrap.current = true;
          wizardResumeHydratedRef.current = resumeKey;
          setWizardResumeLoading(false);
          return;
        }

        if (wizardResumeHydratedRef.current === resumeKey) {
          wizardDbBootstrap.current = true;
          setWizardResumeLoading(false);
          return;
        }

        wizardDbBootstrap.current = true;
        setWizardResumeLoading(entryStep === 4 || entryStep === 5);
        try {
          const loaded = await loadWizardScan(lockedScanId);
          const next = await hydrateWizardSessionForScanResume(session, loaded, {
            lockedProjectId,
            lockedTargetId,
            entryStep,
          });
          const merged = withNormalizedAttackPlan(
            peekWizardSession()?.draftScanId === lockedScanId
              ? mergeWizardSessions(peekWizardSession()!, next)
              : next,
          );
          const projectId = lockedProjectId || loaded.scan.project_id;
          storeDraftScanId(projectId, lockedScanId);
          setSession(merged);
          saveWizardSession(merged);
          wizardResumeHydratedRef.current = resumeKey;
          await actions.refresh();
        } catch (err) {
          wizardDbBootstrap.current = false;
          const message = err instanceof Error ? err.message : "Failed to load wizard scan";
          notify(message, "error");
        } finally {
          setWizardResumeLoading(false);
        }
        return;
      }

      const projectId = lockedProjectId || session.selectedProjectId;
      if (!projectId) return;

      if (session.draftScanId) {
        storeDraftScanId(projectId, session.draftScanId);
        wizardDbBootstrap.current = true;
        const params = new URLSearchParams({ projectId, scanId: session.draftScanId });
        appendWizardUrlParams(params);
        navigate(`/scans/new?${params.toString()}`, { replace: true });
        return;
      }

      wizardDbBootstrap.current = true;
      try {
        const scanId = await resolveOrCreateDraftScanId(projectId, async () => {
          const created = await createWizardScan({
            projectId,
            targetId: session.savedTargetId,
            wizard: wizardStateToPersisted({
              ...createInitialSession(projectId),
              selectedProjectId: projectId,
              draftScanId: null,
            }),
          });
          return created.id;
        });
        applyDraftScanId(scanId, projectId);
        const params = new URLSearchParams({ projectId, scanId });
        appendWizardUrlParams(params);
        navigate(`/scans/new?${params.toString()}`, { replace: true });
        await actions.refresh();
      } catch (err) {
        wizardDbBootstrap.current = false;
        const message = err instanceof Error ? err.message : "Failed to create wizard scan";
        notify(message, "error");
      }
    }

    void ensureDraftScan();
  }, [
    lockedScanId,
    lockedProjectId,
    lockedTargetId,
    requestedStep,
    session.draftScanId,
    session.selectedProjectId,
    session.savedTargetId,
    session.attackPlan,
    session.submittedScanId,
    session.targetProfile.verification.verified,
    targets,
    scans,
    actions,
    notify,
    navigate,
    applyDraftScanId,
    entryStep,
  ]);

  useEffect(() => {
    if (!session.draftScanId || !session.selectedProjectId) return;
    if (wizardSaveTimerRef.current) {
      window.clearTimeout(wizardSaveTimerRef.current);
    }
    wizardSaveTimerRef.current = window.setTimeout(() => {
      void saveWizardScan({
        scanId: session.draftScanId!,
        projectId: session.selectedProjectId,
        targetId: session.savedTargetId,
        wizard: wizardStateToPersisted(session),
      }).then(() => actions.refresh());
    }, 600);
    return () => {
      if (wizardSaveTimerRef.current) {
        window.clearTimeout(wizardSaveTimerRef.current);
        wizardSaveTimerRef.current = null;
      }
      const snapshot = sessionRef.current;
      if (!snapshot.draftScanId || !snapshot.selectedProjectId) return;
      void saveWizardScan({
        scanId: snapshot.draftScanId,
        projectId: snapshot.selectedProjectId,
        targetId: snapshot.savedTargetId,
        wizard: wizardStateToPersisted(snapshot),
      });
    };
  }, [session, actions]);

  useEffect(() => {
    if (session.currentStep < 3 || !session.savedTargetId) return;
    const hydrationKey = `${session.savedTargetId}:${session.draftScanId ?? ""}:${session.currentStep}`;
    if (authHydratedKeyRef.current === hydrationKey) return;

    let cancelled = false;
    void prepareAuthFormForStep3(
      session.targetProfile,
      session.targetForm,
      session.savedTargetId,
    ).then((targetForm) => {
      if (cancelled) return;
      authHydratedKeyRef.current = hydrationKey;
      setSession((prev) => {
        if (targetFormFingerprint(prev.targetForm) === targetFormFingerprint(targetForm)) {
          return prev;
        }
        const next = {
          ...prev,
          targetForm,
          savedTargetFingerprint: targetFormFingerprint(targetForm),
        };
        saveWizardSession(next);
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [session.currentStep, session.savedTargetId, session.draftScanId]);

  const runAttackPlanner = useCallback(
    async (targetId: string, options?: { replan?: boolean }) => {
      const replan = options?.replan ?? false;
      setPlannerReplanning(replan);
      setPlannerGenerating(true);
      setPlannerError(null);
      try {
        const dto = await generateAttackPlanForTarget(targetId);
        const plan = attackPlanFromDto(dto);
        plannerRunRef.current = targetId;
        setSession((prev) => {
          const next = {
            ...prev,
            attackPlan: plan,
            attackPlanUi: attackPlanUiBaselineFromPlan(plan),
          };
          saveWizardSession(next);
          return next;
        });
        return plan;
      } catch (err) {
        const message = toAppError(err).message || "Attack plan generation failed";
        setPlannerError(message);
        notify(message, "error");
        plannerRunRef.current = null;
        return null;
      } finally {
        setPlannerGenerating(false);
        setPlannerReplanning(false);
      }
    },
    [notify],
  );

  const refreshDbVerification = useCallback(async (targetId: string) => {
    setDbVerificationLoading(true);
    try {
      const dto = await getTargetProfile(targetId);
      setDbVerification(profileFromDto(dto).verification);
    } catch {
      setDbVerification(null);
    } finally {
      setDbVerificationLoading(false);
    }
  }, []);

  useEffect(() => {
    if (session.currentStep !== 3 || !session.savedTargetId) {
      setDbVerification(null);
      return;
    }
    void refreshDbVerification(session.savedTargetId);
  }, [session.currentStep, session.savedTargetId, refreshDbVerification]);

  useEffect(() => {
    if (session.currentStep !== 4 || !store.savedTarget) return;
    if (!session.targetProfile.verification.verified) return;
    if (session.attackPlan || plannerGenerating) return;
    void runAttackPlanner(store.savedTarget.id, { replan: false });
  }, [
    session.currentStep,
    store.savedTarget,
    session.targetProfile.verification.verified,
    session.attackPlan,
    plannerGenerating,
    runAttackPlanner,
  ]);

  const step3VerificationBadge = useMemo(() => {
    if (!dbVerification) return null;
    return verificationBadgeFromDb(dbVerification);
  }, [dbVerification]);

  useEffect(() => {
    if (!lockedProjectId) {
      setResolvedProject(null);
      setResolveError(null);
      return;
    }

    if (storeProject) {
      setResolvedProject(null);
      setResolveError(null);
      dispatch({ type: "SET_SELECTED_PROJECT", projectId: storeProject.id });
      return;
    }

    if (loading) return;

    let cancelled = false;
    setResolveError(null);

    void getProject(lockedProjectId)
      .then((dto) => {
        if (cancelled) return;
        const project = mapProjects([dto], [], [])[0];
        setResolvedProject(project);
        dispatch({ type: "SET_SELECTED_PROJECT", projectId: project.id });
      })
      .catch((err) => {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : "Project not found";
        setResolveError(message);
        setResolvedProject(null);
      });

    return () => {
      cancelled = true;
    };
  }, [lockedProjectId, storeProject, loading, dispatch]);

  useEffect(() => {
    if (entryStep !== 2 || lockedTargetId || !lockedProjectId) return;

    const entryKey = `${lockedProjectId}|2`;
    if (newTargetEntryKeyRef.current === entryKey) return;
    newTargetEntryKeyRef.current = entryKey;

    wizardDbBootstrap.current = false;
    deepLinkApplied.current = true;
    clearWizardSession();
    const next = applyWizardEntryStep(createInitialSession(lockedProjectId), 2);
    setSession(next);
    saveWizardSession(next);
    dispatch({ type: "SET_SELECTED_PROJECT", projectId: lockedProjectId });
  }, [entryStep, lockedTargetId, lockedProjectId, dispatch]);

  useEffect(() => {
    if (lockedScanId) return;
    if (deepLinkTargetId.current !== lockedTargetId) {
      deepLinkTargetId.current = lockedTargetId || null;
      deepLinkApplied.current = false;
    }
    if (!lockedTargetId || deepLinkApplied.current) return;

    let cancelled = false;
    const resumeStep: WizardStepId = entryStep ?? 3;

    void loadTargetDtoForWizard(lockedTargetId)
      .then((dto) => {
        if (cancelled) return;
        const target = mapTargets([dto])[0];
        if (lockedProjectId && target.projectId !== lockedProjectId) {
          notify("Target does not belong to this project", "error");
          return;
        }

        const existing = peekWizardSession();
        const sessionMatches =
          resumeStep !== 2 &&
          existing?.savedTargetId === lockedTargetId &&
          (!lockedProjectId ||
            !existing.selectedProjectId ||
            existing.selectedProjectId === lockedProjectId);

        const profileState =
          dto.profile && typeof dto.profile === "object"
            ? profileFromDto(dto.profile as Parameters<typeof profileFromDto>[0])
            : sessionMatches
              ? existing!.targetProfile
              : session.targetProfile;
        const profileUrl = fullProfileUrl(profileState);
        const descriptorForm = targetFormFromDescriptor(dto.descriptor, profileUrl);
        const targetForm = sessionMatches ? existing!.targetForm : descriptorForm;

        const projectId = lockedProjectId || target.projectId;
        const base = sessionMatches
          ? {
              ...existing!,
              draftScanId: entryStep === 2 ? null : existing!.draftScanId,
              selectedProjectId: projectId,
              currentStep: entryStep ?? existing!.currentStep,
              submittedScanId: entryStep === 2 ? null : existing!.submittedScanId,
              attackPlan: entryStep === 2 ? null : existing!.attackPlan,
              targetProfile: profileState,
              targetForm,
              savedTargetFingerprint: targetFormFingerprint(targetForm),
            }
          : {
              ...createSessionForTargetScan(
                projectId,
                target,
                dto.descriptor,
                dto.profile,
                resumeStep,
              ),
              targetForm,
              savedTargetFingerprint: targetFormFingerprint(targetForm),
            };
        const next = entryStep ? applyWizardEntryStep(base, entryStep) : base;
        deepLinkApplied.current = true;
        wizardDbBootstrap.current = entryStep === 2 ? false : wizardDbBootstrap.current;
        setSession(next);
        saveWizardSession(next);
        dispatch({ type: "SET_SELECTED_PROJECT", projectId: next.selectedProjectId });

        if (next.currentStep >= 3) {
          void prepareAuthFormForStep3(profileState, targetForm, lockedTargetId).then(
            (hydrated) => {
              if (targetFormFingerprint(hydrated) === targetFormFingerprint(targetForm)) return;
              setSession((prev) => {
                const merged = {
                  ...prev,
                  targetForm: hydrated,
                  savedTargetFingerprint: targetFormFingerprint(hydrated),
                };
                saveWizardSession(merged);
                return merged;
              });
            },
          );
        }
      })
      .catch((err) => {
        if (!cancelled) {
          const message = err instanceof Error ? err.message : "Failed to load target";
          notify(message || "Target not found — start a new scan manually", "error");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [lockedTargetId, lockedProjectId, lockedScanId, requestedStep, dispatch, notify, session.targetProfile]);

  useEffect(() => {
    if (!session.submittedScanId || session.currentStep < 5) return;
    const timer = window.setInterval(() => void actions.refresh(), 3000);
    return () => window.clearInterval(timer);
  }, [session.submittedScanId, session.currentStep, actions]);

  const submittedStatuses = useScanStatuses(
    session.submittedScanId ? [session.submittedScanId] : [],
    (session.currentStep === 5 || session.currentStep === 6) && session.submittedScanId !== null,
  );
  const submittedLiveStatus = session.submittedScanId
    ? submittedStatuses.get(session.submittedScanId)
    : undefined;
  const submittedStatus = session.submittedScanId
    ? mergeScanStatus(session.submittedScanId, "running", submittedLiveStatus, 0)
    : null;

  const stepDef = getWizardStep(session.currentStep);
  const activeScanId = session.submittedScanId ?? session.draftScanId;
  const targetEndpointUrl =
    fullProfileUrl(session.targetProfile) || store.savedTarget?.url || "";
  const showScanContextHeader =
    (session.currentStep === 5 || session.currentStep === 6) && activeScanId !== null;
  const pageHeaderTitle = showScanContextHeader
    ? `Scan ID: ${activeScanId}`
    : "New Scan";
  const pageHeaderDescription = showScanContextHeader ? (
    targetEndpointUrl ? (
      <span className="wizard-planner-summary page-header__endpoint-summary">
        <strong>AI API Endpoint:</strong>{" "}
        <span className="wizard-planner-summary__url mono">{targetEndpointUrl}</span>
      </span>
    ) : (
      "AI API Endpoint"
    )
  ) : (
    "Configure a new security scan"
  );
  const resultsFindingsCount = useMemo(
    () =>
      session.submittedScanId
        ? findings.filter((finding) => finding.scanId === session.submittedScanId).length
        : 0,
    [findings, session.submittedScanId],
  );
  const showFooterNext =
    session.currentStep < 5 &&
    (session.currentStep !== 4 ||
      (draft.attackPlanGenerated && !plannerGenerating));
  const showStartScan = session.currentStep === 5 && session.submittedScanId === null;
  const showViewResult =
    session.currentStep === 5 &&
    submittedStatus?.status === "completed" &&
    session.submittedScanId === null;
  const showRetryScan =
    session.currentStep === 5 &&
    submittedStatus !== null &&
    (submittedStatus.status === "failed" ||
      submittedStatus.status === "stopped" ||
      submittedStatus.status === "cancelled");
  const attackScanActive =
    session.currentStep === 5 &&
    session.submittedScanId !== null &&
    submittedStatus !== null &&
    ["running", "paused", "pending"].includes(submittedStatus.status);
  const showAttackScanControls = attackScanActive;
  const scanCompleted = submittedStatus?.status === "completed";
  const showHeaderCancel =
    !(session.currentStep === 5 && session.submittedScanId !== null) &&
    !(session.currentStep === 6 && scanCompleted);
  const hideBack =
    session.currentStep === 6 ||
    (session.currentStep === 5 && session.submittedScanId !== null);
  const nextDisabled =
    !canProceedFromStep(session.currentStep, draft) ||
    plannerGenerating ||
    planAdjusting;
  const startScanDisabled = !canStartScan(draft) || startingScan;

  function patchTargetProfile(patch: Partial<typeof session.targetProfile>) {
    setProfileStepError(null);
    setSession((prev) => {
      const next = { ...prev, targetProfile: { ...prev.targetProfile, ...patch } };
      saveWizardSession(next);
      return next;
    });
  }

  function patchTargetForm(patch: Partial<TargetFormState>) {
    setVerificationError(null);
    setSession((prev) => {
      const next = { ...prev, targetForm: { ...prev.targetForm, ...patch } };
      saveWizardSession(next);
      return next;
    });
  }

  async function persistProfileTarget(): Promise<Target | null> {
    const validationError = validateTargetProfile(session.targetProfile);
    if (validationError) {
      setProfileStepError(validationError);
      return null;
    }

    setPersistingTarget(true);
    setProfileStepError(null);
    try {
      const url = fullProfileUrl(session.targetProfile);
      const descriptor = { url, baseUrl: session.targetProfile.baseUrl.trim() };
      const name = deriveTargetNameFromProfile(session.targetProfile);

      let target = store.savedTarget;
      if (!target) {
        target = await actions.createTarget(activeProjectId, name, "llm_api", descriptor);
        updateSession({ savedTargetId: target.id });
      }

      await saveTargetProfile(target.id, profileToPayload(session.targetProfile));
      notify(`Target profile saved for "${name}"`, "success");
      return target;
    } catch (err) {
      const message = toAppError(err).message || "Failed to save target profile";
      setProfileStepError(message);
      notify(message, "error");
      return null;
    } finally {
      setPersistingTarget(false);
    }
  }

  async function persistAuthDescriptor(): Promise<boolean> {
    if (!store.savedTarget) {
      setVerificationError("Complete Step 2 and save the target profile before verifying.");
      return false;
    }
    const validationError = validateTargetStep(session.targetForm);
    if (validationError) {
      setVerificationError(validationError);
      return false;
    }

    setPersistingTarget(true);
    try {
      const url = fullProfileUrl(session.targetProfile);
      const descriptor = buildTargetDescriptor({ ...session.targetForm, url });
      await updateTargetDescriptor(store.savedTarget.id, descriptor);
      updateSession({ savedTargetFingerprint: targetFormFingerprint(session.targetForm) });
      return true;
    } catch (err) {
      const message = toAppError(err).message || "Failed to save authentication";
      setVerificationError(message);
      notify(message, "error");
      return false;
    } finally {
      setPersistingTarget(false);
    }
  }

  async function navigateToStep(nextStep: WizardStepId) {
    setProfileStepError(null);
    setVerificationError(null);

    const targetId = session.savedTargetId;
    const fallbackUrl = session.targetForm.url || store.savedTarget?.url || "";

    if (
      nextStep === 2 &&
      targetId &&
      targetFormNeedsSecretHydration(session.targetForm)
    ) {
      try {
        const targetForm = await fetchTargetFormForWizard(targetId, fallbackUrl);
        updateSession({
          currentStep: nextStep,
          targetForm,
          savedTargetFingerprint: targetFormFingerprint(targetForm),
        });
        return;
      } catch {
        // Fall through — show step with whatever form state we have.
      }
    }

    if (nextStep === 3) {
      try {
        const reinferFromProfile = session.currentStep === 2;
        const targetForm = await prepareAuthFormForStep3(
          session.targetProfile,
          session.targetForm,
          targetId,
          { reinferFromProfile },
        );
        updateSession({
          currentStep: nextStep,
          targetForm,
          savedTargetFingerprint: targetFormFingerprint(targetForm),
          ...(reinferFromProfile
            ? {
                verificationLog: [],
                targetProfile: {
                  ...session.targetProfile,
                  verification: createEmptyVerification(),
                },
              }
            : {}),
        });
      } catch {
        updateSession({ currentStep: nextStep });
      }
      return;
    }

    updateSession({ currentStep: nextStep });
  }

  function handleStepChange(step: WizardStepId) {
    if (canNavigateToStep(step, draft)) {
      void navigateToStep(step);
    }
  }

  async function handleNext() {
    if (session.currentStep >= 6) return;

    if (session.currentStep === 2) {
      const target = await persistProfileTarget();
      if (!target) return;
      const targetForm = await prepareAuthFormForStep3(
        session.targetProfile,
        session.targetForm,
        target.id,
        { reinferFromProfile: true },
      );
      updateSession({
        currentStep: 3,
        targetForm,
        savedTargetFingerprint: targetFormFingerprint(targetForm),
        verificationLog: [],
        targetProfile: {
          ...session.targetProfile,
          verification: createEmptyVerification(),
        },
      });
      return;
    }

    if (session.currentStep === 3) {
      if (!session.targetProfile.verification.verified) {
        setVerificationError("Verify the target connection before continuing.");
        return;
      }
      const saved = await persistAuthDescriptor();
      if (!saved || !store.savedTarget) return;
      updateSession({ currentStep: 4 });
      return;
    }

    if (session.currentStep === 4) {
      if (!canProceedFromStep(4, draft)) return;
      updateSession({ currentStep: 5 });
      if (canStartScan(draft)) {
        await submitScanJob();
      }
      return;
    }

    if (!canProceedFromStep(session.currentStep, draft)) return;
    updateSession({ currentStep: (session.currentStep + 1) as WizardStepId });
  }

  function goToResultsStep() {
    updateSession({ currentStep: 6 });
  }

  function handleBack() {
    if (session.currentStep === 6) return;
    if (session.currentStep > 1) {
      void navigateToStep((session.currentStep - 1) as WizardStepId);
    }
  }

  function handleCancel() {
    navigate("/scans");
  }

  async function handleScanPause() {
    if (!session.submittedScanId) return;
    setScanControlPending(true);
    try {
      await pauseScan(session.submittedScanId);
      notify("Scan paused", "success");
      await actions.refresh();
    } catch (err) {
      notify(toAppError(err).message || "Failed to pause scan", "error");
    } finally {
      setScanControlPending(false);
    }
  }

  async function handleScanResume() {
    if (!session.submittedScanId) return;
    setScanControlPending(true);
    try {
      await resumeScan(session.submittedScanId);
      notify("Scan resumed", "success");
      await actions.refresh();
    } catch (err) {
      notify(toAppError(err).message || "Failed to resume scan", "error");
    } finally {
      setScanControlPending(false);
    }
  }

  async function handleStartScan() {
    if (!canStartScan(draft) || !store.savedTarget || !session.attackPlan) return;
    await submitScanJob();
  }

  async function submitScanJob(options?: { restart?: boolean }) {
    if (!store.savedTarget || !session.attackPlan) return;
    if (startingScanRef.current) return;

    const scanIdToReuse = session.submittedScanId ?? session.draftScanId ?? undefined;

    if (!options?.restart) {
      updateSession({ currentStep: 5 });
      await new Promise<void>((resolve) => {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => resolve());
        });
      });
    }

    if (scanIdToReuse && !options?.restart) {
      try {
        const live = await getScanStatus(scanIdToReuse);
        if (["running", "paused", "pending"].includes(live.status)) {
          updateSession({ submittedScanId: scanIdToReuse, currentStep: 5 });
          return;
        }
      } catch {
        // Continue and attempt a fresh start below.
      }
    }

    startingScanRef.current = true;
    setStartingScan(true);
    setScanSubmitError(null);
    try {
      const result = await startScan({
        projectId: activeProjectId,
        targetId: store.savedTarget.id,
        categories: session.attackPlan.categories,
        profile: session.attackPlan.profileId,
        disabledTests: session.attackPlan.disabledTests,
        payloadStrategy: payloadStrategyToDto(session.attackPlan.payloadStrategy),
        agentMode: session.attackPlan.executionStrategy === "agentic",
        maxAgentAttempts: session.attackPlan.maxAttempts,
        reflectionEnabled: session.attackPlan.reflectionEnabled,
        draftScanId: scanIdToReuse,
      });
      await actions.refresh();
      updateSession({ submittedScanId: result.scan_id, currentStep: 5 });
      notify(options?.restart ? "Attack restarted" : "Attack started in the background", "success");
    } catch (err) {
      const message = toAppError(err).message || "Failed to start scan";
      setScanSubmitError(message);
      notify(message, "error");
    } finally {
      startingScanRef.current = false;
      setStartingScan(false);
    }
  }

  async function handleRetryScan() {
    if (!store.savedTarget || !session.attackPlan) return;
    await submitScanJob({ restart: true });
  }

  function renderStepBody() {
    switch (session.currentStep) {
      case 1:
        return (
          <ProjectStep
            lockedProjectId={lockedProjectId}
            lockedProject={lockedProject}
            resolveError={resolveError}
            loading={loading}
            projects={projects}
            selectedProjectId={session.selectedProjectId}
            onSelectProject={(projectId) => {
              updateSession({ selectedProjectId: projectId });
              dispatch({ type: "SET_SELECTED_PROJECT", projectId: projectId || null });
            }}
          />
        );
      case 2:
        return activeProjectId ? (
          <TargetProfileStep
            profile={session.targetProfile}
            onChange={patchTargetProfile}
            error={profileStepError}
            hasPersistedTarget={session.savedTargetId !== null}
          />
        ) : (
          <p className="text-muted">Select a project in step 1 to configure the AI target profile.</p>
        );
      case 3:
        return store.savedTarget ? (
          <>
            <AuthVerificationStep
            targetId={store.savedTarget.id}
            profile={session.targetProfile}
            onProfileChange={patchTargetProfile}
            authForm={session.targetForm}
            onAuthChange={patchTargetForm}
            verificationLog={session.verificationLog}
            onVerificationLog={(entries) => updateSession({ verificationLog: entries })}
            onError={setVerificationError}
            onBeforeVerify={persistAuthDescriptor}
            onVerifySettled={() => {
              if (store.savedTarget) void refreshDbVerification(store.savedTarget.id);
            }}
            onVerifySuccess={() => {
              if (!store.savedTarget) return;
              updateSession({ attackPlan: null, attackPlanUi: createInitialAttackPlanUi() });
              plannerRunRef.current = null;
            }}
          />
          </>
        ) : (
          <p className="text-muted">Complete step 2 to configure authentication.</p>
        );
      case 4:
        if (wizardResumeLoading) {
          return <p className="text-muted">Loading attack plan…</p>;
        }
        if (!session.targetProfile.verification.verified) {
          return <p className="text-muted">Waiting for target verification…</p>;
        }
        if (plannerGenerating || (!session.attackPlan && !plannerError)) {
          return <AttackPlanPlanningState replanning={plannerReplanning} />;
        }
        if (plannerError && !session.attackPlan) {
          return (
            <div>
              <p className="text-danger">{plannerError}</p>
              {store.savedTarget && (
                <Button
                  variant="primary"
                  onClick={() => void runAttackPlanner(store.savedTarget!.id, { replan: false })}
                >
                  Retry planning
                </Button>
              )}
            </div>
          );
        }
        return store.savedTarget && session.attackPlan ? (
          <ReviewAttackPlanStep
            targetId={store.savedTarget.id}
            attackPlan={session.attackPlan}
            planUi={session.attackPlanUi}
            onPlanUiChange={(patch) =>
              setSession((prev) => {
                const next = { ...prev, attackPlanUi: { ...prev.attackPlanUi, ...patch } };
                saveWizardSession(next);
                return next;
              })
            }
            onPlanChange={(plan) => updateSession({ attackPlan: plan })}
            onAdjustingChange={setPlanAdjusting}
          />
        ) : (
          <AttackPlanPlanningState replanning={false} />
        );
      case 5: {
        const waitingForTarget =
          Boolean(session.submittedScanId && session.savedTargetId && !store.savedTarget);
        return wizardResumeLoading || waitingForTarget ? (
          <p className="text-muted">Loading attack progress…</p>
        ) : sessionReadyForSubmitStep(session, store.savedTarget) && store.savedTarget ? (
          <>
            <SubmitStep
              target={store.savedTarget}
              targetProfile={session.targetProfile}
              attackPlan={session.attackPlan!}
              submittedScanId={session.submittedScanId}
              onViewResult={goToResultsStep}
              onRetryScan={() => void handleRetryScan()}
              onClose={handleCancel}
            />
            {scanSubmitError && <p className="text-danger">{scanSubmitError}</p>}
          </>
        ) : (
          <p className="text-muted">Complete steps 1–4 before submitting the scan.</p>
        );
      }
      case 6:
        return session.submittedScanId ? (
          <ResultsStep
            scanId={session.submittedScanId}
            attackCategories={session.attackPlan?.categories}
          />
        ) : (
          <p className="text-muted">Submit a scan in step 5 to review results.</p>
        );
      default:
        return null;
    }
  }

  return (
    <div className="page">
      <PageHeader
        title={pageHeaderTitle}
        description={pageHeaderDescription}
        actions={
          showAttackScanControls ? (
            <div className="page-actions">
              {submittedStatus?.status === "running" && (
                <Button
                  variant="secondary"
                  disabled={
                    scanControlPending ||
                    submittedStatus.pause_pending === true
                  }
                  onClick={() => void handleScanPause()}
                >
                  {submittedStatus.pause_pending ? "Pausing…" : "Pause"}
                </Button>
              )}
              {submittedStatus?.status === "paused" && (
                <Button
                  variant="secondary"
                  disabled={scanControlPending}
                  onClick={() => void handleScanResume()}
                >
                  Resume
                </Button>
              )}
            </div>
          ) : showHeaderCancel ? (
            <Button variant="danger" onClick={handleCancel}>
              Cancel
            </Button>
          ) : undefined
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <WizardStepper
        currentStep={session.currentStep}
        draft={draft}
        onStepChange={handleStepChange}
      />

      <Card className="wizard-panel">
        <header className="wizard-panel__header">
          <p className="wizard-panel__step-label text-muted">
            Step {session.currentStep} of 6 · {stepDef.label}
          </p>
          <h2 className="wizard-panel__title">{stepDef.title}</h2>
          <div className="wizard-panel__hint-row">
            <p className="wizard-panel__hint text-muted">{stepDef.hint}</p>
            {session.currentStep === 4 &&
            store.savedTarget &&
            session.targetProfile.verification.verified &&
            session.attackPlan ? (
              <Button
                variant="ghost"
                disabled={plannerGenerating || planAdjusting}
                onClick={() => {
                  plannerRunRef.current = null;
                  updateSession({
                    attackPlan: null,
                    attackPlanUi: createInitialAttackPlanUi(),
                  });
                  void runAttackPlanner(store.savedTarget!.id, { replan: true });
                }}
              >
                {plannerGenerating && plannerReplanning ? "Re-planning…" : "Re-plan"}
              </Button>
            ) : session.currentStep === 4 && plannerGenerating ? (
              <Badge variant="muted">{plannerReplanning ? "Re-planning…" : "Planning…"}</Badge>
            ) : null}
            {session.currentStep === 3 && session.savedTargetId ? (
              dbVerificationLoading ? (
                <Badge variant="muted">Checking…</Badge>
              ) : step3VerificationBadge ? (
                <Badge variant={step3VerificationBadge.variant}>
                  {step3VerificationBadge.label}
                </Badge>
              ) : null
            ) : null}
            {session.currentStep === 2 && activeProjectId ? (
              <Button variant="ghost" onClick={() => setImportApiOpen(true)}>
                Import
              </Button>
            ) : null}
            {session.currentStep === 6 && activeProjectId && session.submittedScanId ? (
              <ReportExportDropdown
                projectId={activeProjectId}
                scanId={session.submittedScanId}
                findingsCount={resultsFindingsCount}
              />
            ) : null}
          </div>
        </header>

        <div className="wizard-panel__body">{renderStepBody()}</div>

        <footer className="wizard-panel__footer">
          {!hideBack ? (
            <Button variant="ghost" disabled={session.currentStep === 1} onClick={handleBack}>
              Back
            </Button>
          ) : (
            <span />
          )}
          <div className="wizard-panel__footer-actions">
            {showStartScan && (
              <Button
                variant="primary"
                disabled={startScanDisabled}
                onClick={() => void handleStartScan()}
              >
                {startingScan ? "Starting attack…" : "Start Attack"}
              </Button>
            )}
            {showViewResult && (
              <Button variant="primary" onClick={goToResultsStep}>
                View Result
              </Button>
            )}
            {showRetryScan && (
              <Button
                variant="primary"
                disabled={startingScan}
                onClick={() => void handleRetryScan()}
              >
                {startingScan ? "Retrying…" : "Retry Attack"}
              </Button>
            )}
            {showFooterNext && (
              <Button
                variant="primary"
                disabled={nextDisabled || persistingTarget || startingScan}
                onClick={() => void handleNext()}
              >
                {session.currentStep === 4
                  ? startingScan
                    ? "Starting attack…"
                    : "Start Attack"
                  : persistingTarget
                    ? "Saving target…"
                    : "Next"}
              </Button>
            )}
          </div>
        </footer>
      </Card>

      <ImportApiModal
        open={importApiOpen}
        onClose={() => setImportApiOpen(false)}
        onImport={(patch) => patchTargetProfile(patch)}
      />
    </div>
  );
}
