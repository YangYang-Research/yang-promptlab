import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { mapProjects, mapTargets } from "@/app/store/mappers";
import { Button, Card, PageHeader, Badge, YazgBadge, IconAi } from "@/shared/components";
import { getProject, getScanStatus, pauseScan, resumeScan, startScan, updateTargetDescriptor } from "@/shared/ipc";
import { createWizardScan, loadWizardScan, saveWizardScan } from "@/shared/ipc/scanWizard";
import { generateAttackPlanForTarget } from "@/shared/ipc/attackPlanner";
import { getTargetProfile, saveTargetProfile } from "@/shared/ipc/targetProfile";
import { toAppError } from "@/shared/errors";
import { useToast } from "@/shared/notifications";
import { assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
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
import { attackPlanFromDto, attackPlanUiBaselineFromPlan, normalizeAttackPlan, payloadStrategyToDto, resolvePlannerSummaryBadge } from "./attackPlan";
import {
  buildWizardStore,
  clearWizardSession,
  createInitialSession,
  createSessionForTargetScan,
  applyWizardEntryStep,
  parseWizardEntryStep,
  isFreshWizardEntry,
  isScanResultsReady,
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
  canReuseWizardDraft,
  resolveOrCreateDraftScanId,
  storeDraftScanId,
} from "./wizardDraftScan";
import { createSessionFromScanConfigImport } from "./scanConfigExport";
import {
  IMPORT_STEP_COUNTDOWN_SEC,
  IMPORT_VERIFY_MAX_ATTEMPTS,
} from "./importHarness";
import { logWizardEvent } from "./wizardLiveLog";

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
  const autoStartRequested = searchParams.get("autoStart") === "1";
  const entryStep = parseWizardEntryStep(requestedStep);
  const isFreshWizard = isFreshWizardEntry({
    scanId: lockedScanId,
    targetId: lockedTargetId,
    step: requestedStep,
  });
  const { projects, targets, scans, findings, loading, error, dispatch, actions } = useAppStore();
  const { notify, dismiss } = useToast();
  const deepLinkApplied = useRef(false);
  const deepLinkTargetId = useRef<string | null>(null);

  const [session, setSession] = useState<ScanWizardSession>(() => {
    if (isFreshWizard) {
      clearWizardSession();
      return createSessionFromScanConfigImport(lockedProjectId);
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
  const [consoleResetKey, setConsoleResetKey] = useState(0);
  const [plannerGenerating, setPlannerGenerating] = useState(false);
  const [plannerReplanning, setPlannerReplanning] = useState(false);
  const [plannerError, setPlannerError] = useState<string | null>(null);
  const [planAdjusting, setPlanAdjusting] = useState(false);
  const [dbVerification, setDbVerification] = useState<VerificationResultForm | null>(null);
  const [dbVerificationLoading, setDbVerificationLoading] = useState(false);
  const [importApiOpen, setImportApiOpen] = useState(false);
  const [importStepCountdown, setImportStepCountdown] = useState<number | null>(null);
  const [importVerifyAttempt, setImportVerifyAttempt] = useState(0);
  const [importPostVerifyCountdown, setImportPostVerifyCountdown] = useState<number | null>(null);
  const importAdvanceLock = useRef(false);
  const importHarnessLoggedRef = useRef(false);
  const [scanControlPending, setScanControlPending] = useState(false);
  const [wizardResumeLoading, setWizardResumeLoading] = useState(() =>
    Boolean(lockedScanId && (entryStep === 4 || entryStep === 5)),
  );
  const plannerRunRef = useRef<string | null>(null);
  const startingScanRef = useRef(false);
  const autoStartTriggeredRef = useRef(false);
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
      const targetId =
        (typeof patch.savedTargetId === "string" && patch.savedTargetId) ||
        lockedTargetId ||
        sessionRef.current.savedTargetId ||
        null;
      storeDraftScanId(projectId, scanId, targetId);
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
    [lockedTargetId],
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
    autoStartTriggeredRef.current = false;
    clearWizardSession();
    setPlannerError(null);
    setPlannerGenerating(false);
    setProfileStepError(null);
    setVerificationError(null);
    setScanSubmitError(null);
    setSession(createSessionFromScanConfigImport(lockedProjectId, { consume: true }));
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

        // Step 5 needs DB status so a draft is not treated as an already-started attack
        // (sessionStorage may still have a polluted submittedScanId).
        if (session.draftScanId === lockedScanId && readyForEntry && entryStep !== 5) {
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
          let merged = withNormalizedAttackPlan(
            peekWizardSession()?.draftScanId === lockedScanId
              ? mergeWizardSessions(peekWizardSession()!, next)
              : next,
          );
          // Authoritative: draft scans are never "submitted" (merge can reintroduce
          // a polluted submittedScanId from sessionStorage).
          if (loaded.scan.status === "draft" && merged.submittedScanId) {
            merged = { ...merged, submittedScanId: null };
          }
          // Retry / replan deep link: Step 4 must not carry the prior failed run as submitted.
          if (entryStep === 4 && merged.submittedScanId) {
            merged = { ...merged, submittedScanId: null };
          }
          const projectId = lockedProjectId || loaded.scan.project_id;
          storeDraftScanId(
            projectId,
            lockedScanId,
            loaded.scan.target_id ?? (lockedTargetId || null),
          );
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

      // Wait until target deep-link hydrates before binding/creating a draft.
      if (lockedTargetId && session.savedTargetId !== lockedTargetId) {
        return;
      }

      const draftScanTargetId =
        scans.find((scan) => scan.id === session.draftScanId)?.targetId ?? null;
      if (
        canReuseWizardDraft({
          draftScanId: session.draftScanId,
          sessionTargetId: session.savedTargetId,
          lockedTargetId,
          entryStep,
          draftScanTargetId,
        })
      ) {
        storeDraftScanId(projectId, session.draftScanId!, lockedTargetId || session.savedTargetId);
        wizardDbBootstrap.current = true;
        const params = new URLSearchParams({ projectId, scanId: session.draftScanId! });
        appendWizardUrlParams(params);
        navigate(`/scans/new?${params.toString()}`, { replace: true });
        return;
      }

      wizardDbBootstrap.current = true;
      const draftTargetId = lockedTargetId || session.savedTargetId;
      try {
        const scanId = await resolveOrCreateDraftScanId(
          projectId,
          async () => {
            const created = await createWizardScan({
              projectId,
              targetId: draftTargetId,
              wizard: wizardStateToPersisted({
                ...createInitialSession(projectId),
                selectedProjectId: projectId,
                savedTargetId: draftTargetId,
                draftScanId: null,
                currentStep: entryStep ?? session.currentStep,
              }),
            });
            return created.id;
          },
          draftTargetId,
        );
        applyDraftScanId(scanId, projectId, {
          savedTargetId: draftTargetId,
          draftScanId: scanId,
        });
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
    session.currentStep,
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
    // Don't re-hydrate auth when returning from step 4+ — keeps verification process intact.
    if (session.currentStep > 3) return;
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
      const yazg = await assertYazgAgentLive(true);
      if (!yazg.live) {
        setPlannerError(yazg.message);
        notify(yazg.message, "error");
        return null;
      }

      const replan = options?.replan ?? false;
      setPlannerReplanning(replan);
      setPlannerGenerating(true);
      setPlannerError(null);
      logWizardEvent({
        category: "planner",
        activityName: replan ? "wizard_plan_replan" : "wizard_plan_request",
        message: replan ? "Re-planning attack plan…" : "Generating attack plan…",
        projectId: sessionRef.current.selectedProjectId || lockedProjectId || null,
        attributes: { targetId, replan },
      });
      try {
        const dto = await generateAttackPlanForTarget(targetId);
        const plan = attackPlanFromDto(dto);
        plannerRunRef.current = targetId;
        setSession((prev) => {
          const next = {
            ...prev,
            attackPlan: plan,
            attackPlanUi: attackPlanUiBaselineFromPlan(plan),
            attackPlanSource: "generated" as const,
          };
          saveWizardSession(next);
          return next;
        });
        logWizardEvent({
          category: "planner",
          activityName: "wizard_plan_ready",
          message: `Attack plan ready (${plan.categories.length} categories, profile ${plan.profileId})`,
          projectId: sessionRef.current.selectedProjectId || lockedProjectId || null,
          attributes: {
            targetId,
            profileId: plan.profileId,
            recommendedProfileId: plan.recommendedProfileId,
            categories: plan.categories.length,
            modes: plan.profileModes.length,
          },
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
    [notify, lockedProjectId],
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
    if (session.attackPlan || plannerGenerating || plannerError) return;
    void runAttackPlanner(store.savedTarget.id, { replan: false });
  }, [
    session.currentStep,
    store.savedTarget,
    session.targetProfile.verification.verified,
    session.attackPlan,
    plannerGenerating,
    plannerError,
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
              attackPlanSource: entryStep === 2 ? null : existing!.attackPlanSource,
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
  const submittedStoreStatus =
    session.submittedScanId != null
      ? (scans.find((scan) => scan.id === session.submittedScanId)?.status ?? "pending")
      : null;
  const submittedStatus = session.submittedScanId
    ? mergeScanStatus(session.submittedScanId, submittedStoreStatus ?? "pending", submittedLiveStatus, 0)
    : null;

  useEffect(() => {
    if (session.currentStep !== 6 || !session.submittedScanId || !submittedStatus) return;
    if (!isScanResultsReady(submittedStatus.status)) {
      updateSession({ currentStep: 5 });
    }
  }, [session.currentStep, session.submittedScanId, submittedStatus, updateSession]);

  const stepDef = getWizardStep(session.currentStep);
  const step4PlannerBadge = useMemo(() => {
    if (session.currentStep !== 4 || !session.attackPlan) return null;
    return resolvePlannerSummaryBadge(session.attackPlan, session.attackPlanUi);
  }, [session.currentStep, session.attackPlan, session.attackPlanUi]);
  const activeScanId = session.submittedScanId ?? session.draftScanId;
  const targetEndpointUrl =
    fullProfileUrl(session.targetProfile) || store.savedTarget?.url || "";
  const showScanContextHeader = session.currentStep >= 2;
  const pageHeaderTitle =
    showScanContextHeader && activeScanId
      ? `Scan ID: ${activeScanId}`
      : showScanContextHeader
        ? "Scan ID"
        : "New Scan";
  const pageHeaderDescription = showScanContextHeader ? (
    targetEndpointUrl ? (
      <span className="wizard-planner-summary page-header__endpoint-summary">
        <strong>Endpoint:</strong>{" "}
        <span className="wizard-planner-summary__url mono">{targetEndpointUrl}</span>
      </span>
    ) : (
      "Endpoint"
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
  const showDone = session.currentStep === 6;
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
  const importHarnessBusy =
    session.importAutoAdvance &&
    session.currentStep <= 3 &&
    (importStepCountdown !== null ||
      importPostVerifyCountdown !== null ||
      (session.currentStep === 3 && !session.targetProfile.verification.verified));
  const nextDisabled =
    !canProceedFromStep(session.currentStep, draft) ||
    plannerGenerating ||
    planAdjusting ||
    importHarnessBusy;
  const startScanDisabled = !canStartScan(draft) || startingScan;

  function importFooterLabel(): string {
    if (importPostVerifyCountdown !== null) {
      return "Importing…";
    }
    if (session.importAutoAdvance && session.currentStep === 3) {
      const attempt = Math.max(importVerifyAttempt, 1);
      return `Verifying… (${attempt}/${IMPORT_VERIFY_MAX_ATTEMPTS})`;
    }
    if (importStepCountdown !== null && session.currentStep <= 2) {
      return "Importing…";
    }
    if (session.currentStep === 4) {
      return "Review Attack";
    }
    if (persistingTarget) return "Saving target…";
    return "Next";
  }

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
      logWizardEvent({
        activityName: "wizard_profile_saved",
        message: `Target profile saved: ${name}`,
        projectId: activeProjectId,
        attributes: { targetId: target.id, endpoint: url },
      });
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
    const url = fullProfileUrl(session.targetProfile);
    const formForSave = {
      ...session.targetForm,
      url: session.targetForm.url.trim() || url,
    };
    const validationError = validateTargetStep(formForSave);
    if (validationError) {
      setVerificationError(validationError);
      notify(validationError, "error");
      return false;
    }

    setPersistingTarget(true);
    try {
      const descriptor = buildTargetDescriptor({ ...formForSave, url });
      await updateTargetDescriptor(store.savedTarget.id, descriptor);
      updateSession({
        targetForm: formForSave,
        savedTargetFingerprint: targetFormFingerprint(formForSave),
      });
      logWizardEvent({
        category: "authentication",
        activityName: "wizard_auth_saved",
        message: "Authentication descriptor saved",
        projectId: activeProjectId,
        attributes: { targetId: store.savedTarget.id, authKind: formForSave.authKind },
      });
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
      // Returning from a later step: keep auth + verification process as-is.
      if (session.currentStep > 3) {
        updateSession({ currentStep: nextStep });
        return;
      }

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

    // Entering Step 5 from earlier wizard steps is Attack review until Start Attack.
    if (nextStep === 5 && session.currentStep < 5) {
      updateSession({ currentStep: 5, submittedScanId: null });
      return;
    }

    updateSession({ currentStep: nextStep });
  }

  function handleStepChange(step: WizardStepId) {
    if (session.importAutoAdvance && session.currentStep <= 3) {
      return;
    }
    if (canNavigateToStep(step, draft, { scanStatus: submittedStatus?.status })) {
      void navigateToStep(step);
    }
  }

  async function handleNext() {
    if (session.currentStep >= 6) return;
    const fromStep = session.currentStep;
    const projectId = activeProjectId || null;
    const harness = session.importAutoAdvance;

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
      logWizardEvent({
        category: harness ? "harness" : "user_interface",
        activityName: "wizard_step_advance",
        message: harness
          ? "Import harness advanced to Authentication"
          : "Advanced to Authentication",
        projectId,
        attributes: { from: fromStep, to: 3, harness, targetId: target.id },
      });
      return;
    }

    if (session.currentStep === 3) {
      if (!session.targetProfile.verification.verified) {
        const message = "Verify the target connection before continuing.";
        setVerificationError(message);
        notify(message, "error");
        return;
      }
      const saved = await persistAuthDescriptor();
      if (!saved || !store.savedTarget) return;
      updateSession({ currentStep: 4 });
      logWizardEvent({
        category: harness ? "harness" : "user_interface",
        activityName: "wizard_step_advance",
        message: harness
          ? "Import harness advanced to Attack Plan"
          : "Advanced to Attack Plan",
        projectId,
        attributes: { from: fromStep, to: 4, harness },
      });
      return;
    }

    if (session.currentStep === 4) {
      if (!canProceedFromStep(4, draft)) return;
      // Always enter Step 5 as Attack review (not prior run monitor). Start Attack
      // sets submittedScanId when the user launches.
      updateSession({ currentStep: 5, submittedScanId: null });
      logWizardEvent({
        activityName: "wizard_step_advance",
        message: "Advanced to Attack review",
        projectId,
        attributes: { from: fromStep, to: 5 },
      });
      return;
    }

    if (!canProceedFromStep(session.currentStep, draft)) return;
    const toStep = (session.currentStep + 1) as WizardStepId;
    updateSession({ currentStep: toStep });
    logWizardEvent({
      category: harness ? "harness" : "user_interface",
      activityName: "wizard_step_advance",
      message: `Advanced to step ${toStep}`,
      projectId,
      attributes: { from: fromStep, to: toStep, harness },
    });
  }

  function goToResultsStep() {
    if (!isScanResultsReady(submittedStatus?.status)) return;
    updateSession({ currentStep: 6 });
  }

  function handleBack() {
    if (session.currentStep === 6) return;
    const fromStep = session.currentStep;
    if (session.importAutoAdvance) {
      updateSession({ importAutoAdvance: false });
      setImportStepCountdown(null);
      setImportPostVerifyCountdown(null);
      setImportVerifyAttempt(0);
      logWizardEvent({
        category: "harness",
        severity: "low",
        activityName: "wizard_import_cancelled",
        message: "Import harness cancelled by user",
        projectId: activeProjectId || null,
        attributes: { step: fromStep },
      });
    }
    if (session.currentStep > 1) {
      void navigateToStep((session.currentStep - 1) as WizardStepId);
      logWizardEvent({
        category: "user_interface",
        activityName: "wizard_step_back",
        message: `Returned to step ${fromStep - 1}`,
        projectId: activeProjectId || null,
        attributes: { from: fromStep, to: fromStep - 1 },
      });
    }
  }

  const importAutoActive = session.importAutoAdvance;
  const canImportStepAdvance =
    importAutoActive &&
    (session.currentStep === 1 || session.currentStep === 2) &&
    canProceedFromStep(session.currentStep, draft) &&
    !persistingTarget &&
    importPostVerifyCountdown === null;

  // Log once when import harness is armed.
  useEffect(() => {
    if (!session.importAutoAdvance) {
      importHarnessLoggedRef.current = false;
      return;
    }
    if (importHarnessLoggedRef.current) return;
    importHarnessLoggedRef.current = true;
    logWizardEvent({
      category: "harness",
      activityName: "wizard_import_start",
      message: "Import harness started — auto-walking wizard steps",
      projectId: activeProjectId || session.selectedProjectId || null,
      attributes: {
        step: session.currentStep,
        hasPlan: Boolean(session.attackPlan),
        planSource: session.attackPlanSource,
      },
    });
  }, [
    session.importAutoAdvance,
    session.currentStep,
    session.attackPlan,
    session.attackPlanSource,
    session.selectedProjectId,
    activeProjectId,
  ]);

  // Import harness: countdown then auto Next on steps 1 and 2.
  useEffect(() => {
    if (!canImportStepAdvance) {
      if (!importAutoActive || session.currentStep > 2) {
        setImportStepCountdown(null);
      }
      return;
    }

    let remaining = IMPORT_STEP_COUNTDOWN_SEC;
    setImportStepCountdown(remaining);
    const timer = window.setInterval(() => {
      remaining -= 1;
      if (remaining <= 0) {
        window.clearInterval(timer);
        setImportStepCountdown(0);
        if (importAdvanceLock.current) return;
        importAdvanceLock.current = true;
        void handleNext().finally(() => {
          importAdvanceLock.current = false;
        });
        return;
      }
      setImportStepCountdown(remaining);
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
    // handleNext closes over latest session; re-arm only when step/gates change.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- import step gate
  }, [canImportStepAdvance, session.currentStep, importAutoActive]);

  // Import harness: after verify success, countdown then advance to step 4.
  useEffect(() => {
    if (importPostVerifyCountdown === null) return;

    if (importPostVerifyCountdown <= 0) {
      if (importAdvanceLock.current) return;
      importAdvanceLock.current = true;
      setImportPostVerifyCountdown(null);
      void (async () => {
        try {
          updateSession({ importAutoAdvance: false });
          await handleNext();
          logWizardEvent({
            category: "harness",
            activityName: "wizard_import_complete",
            message: "Import complete — review the attack plan, then start the attack",
            projectId: activeProjectId || null,
            attributes: {
              hasPlan: Boolean(session.attackPlan),
              planSource: session.attackPlanSource,
            },
          });
          notify(
            "Import successful. Review the attack plan, then start the attack.",
            "success",
          );
        } finally {
          importAdvanceLock.current = false;
        }
      })();
      return;
    }

    const timer = window.setTimeout(() => {
      setImportPostVerifyCountdown((prev) => (prev === null ? null : prev - 1));
    }, 1000);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- post-verify gate
  }, [importPostVerifyCountdown, updateSession, notify]);

  function handleCancel() {
    navigate("/scans");
  }

  async function handleScanPause() {
    if (!session.submittedScanId) return;
    setScanControlPending(true);
    let pendingToastId: number | undefined;
    try {
      pendingToastId = notify("Pausing scan…", "info");
      await pauseScan(session.submittedScanId);
      dismiss(pendingToastId);
      pendingToastId = undefined;
      notify("Scan paused", "success");
      await actions.refresh();
    } catch (err) {
      if (pendingToastId !== undefined) dismiss(pendingToastId);
      notify(toAppError(err).message || "Failed to pause scan", "error");
    } finally {
      setScanControlPending(false);
    }
  }

  async function handleScanResume() {
    if (!session.submittedScanId) return;
    setScanControlPending(true);
    let pendingToastId: number | undefined;
    try {
      pendingToastId = notify("Resuming scan…", "info");
      await resumeScan(session.submittedScanId);
      dismiss(pendingToastId);
      pendingToastId = undefined;
      notify("Scan resumed", "success");
      await actions.refresh();
    } catch (err) {
      if (pendingToastId !== undefined) dismiss(pendingToastId);
      notify(toAppError(err).message || "Failed to resume scan", "error");
    } finally {
      setScanControlPending(false);
    }
  }

  async function handleStartScan() {
    if (!canStartScan(draft) || !store.savedTarget || !session.attackPlan) return;
    await submitScanJob();
  }

  async function submitScanJob(options?: { restart?: boolean; retryFailedOnly?: boolean }) {
    if (!store.savedTarget || !session.attackPlan) return;
    if (startingScanRef.current) return;

    const scanIdToReuse = session.submittedScanId ?? session.draftScanId ?? undefined;

    if (!options?.restart && !options?.retryFailedOnly) {
      updateSession({ currentStep: 5 });
      await new Promise<void>((resolve) => {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => resolve());
        });
      });
    }

    if (scanIdToReuse && !options?.restart && !options?.retryFailedOnly) {
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
    const activityLabel = options?.retryFailedOnly
      ? "Retrying failed categories…"
      : options?.restart
        ? "Restarting attack…"
        : "Starting attack…";
    logWizardEvent({
      activityName: "wizard_attack_start",
      message: activityLabel,
      projectId: activeProjectId || null,
      scanId: scanIdToReuse ?? null,
      attributes: {
        targetId: store.savedTarget.id,
        profileId: session.attackPlan.profileId,
        categories: session.attackPlan.categories.length,
        restart: Boolean(options?.restart),
        retryFailedOnly: Boolean(options?.retryFailedOnly),
      },
    });
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
        adaptivePlanning: session.attackPlan.adaptivePlanning,
        draftScanId: scanIdToReuse,
        retryFailedOnly: options?.retryFailedOnly,
      });
      await actions.refresh();
      updateSession({ submittedScanId: result.scan_id, currentStep: 5 });
      if (options?.restart && !options?.retryFailedOnly) {
        setConsoleResetKey((key) => key + 1);
      }
      logWizardEvent({
        activityName: "wizard_attack_started",
        message: options?.retryFailedOnly
          ? `Failed-category retry started (${result.scan_id})`
          : `Attack started (${result.scan_id})`,
        projectId: activeProjectId || null,
        scanId: result.scan_id,
      });
      notify(
        options?.retryFailedOnly
          ? "Retrying failed categories"
          : options?.restart
            ? "Attack restarted"
            : "Attack started in the background",
        "success",
      );
    } catch (err) {
      const message = toAppError(err).message || "Failed to start scan";
      setScanSubmitError(message);
      logWizardEvent({
        severity: "high",
        activityName: "wizard_attack_start_failed",
        message,
        projectId: activeProjectId || null,
        scanId: scanIdToReuse ?? null,
      });
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

  async function handleRetryFailedCategories() {
    if (!store.savedTarget || !session.attackPlan) return;
    await submitScanJob({ retryFailedOnly: true });
  }

  useEffect(() => {
    if (!autoStartRequested || wizardResumeLoading || autoStartTriggeredRef.current) return;
    if (!store.savedTarget || !session.attackPlan) return;

    autoStartTriggeredRef.current = true;
    const params = new URLSearchParams(searchParams);
    params.delete("autoStart");
    navigate(`/scans/new?${params.toString()}`, { replace: true });
    void handleRetryScan();
  }, [
    autoStartRequested,
    wizardResumeLoading,
    store.savedTarget,
    session.attackPlan,
    searchParams,
    navigate,
  ]);

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
            error={verificationError}
            onBeforeVerify={persistAuthDescriptor}
            autoVerify={
              session.importAutoAdvance &&
              !session.targetProfile.verification.verified &&
              importPostVerifyCountdown === null
            }
            autoVerifyMaxAttempts={IMPORT_VERIFY_MAX_ATTEMPTS}
            onAutoVerifyAttempt={(attempt) => setImportVerifyAttempt(attempt)}
            onAutoVerifyComplete={(result) => {
              if (result.ok) {
                setImportPostVerifyCountdown(IMPORT_STEP_COUNTDOWN_SEC);
                return;
              }
              updateSession({ importAutoAdvance: false });
              setImportVerifyAttempt(0);
              logWizardEvent({
                category: "harness",
                severity: "high",
                activityName: "wizard_import_verify_failed",
                message: result.message
                  ? `Import verification failed after ${result.attempts} attempts: ${result.message}`
                  : `Import verification failed after ${result.attempts} attempts`,
                projectId: activeProjectId || null,
                attributes: { attempts: result.attempts },
              });
              notify(
                result.message
                  ? `Import verification failed after ${result.attempts} attempts: ${result.message}`
                  : `Import verification failed after ${result.attempts} attempts. Fix credentials and verify manually.`,
                "error",
              );
            }}
            onVerifySettled={() => {
              if (store.savedTarget) void refreshDbVerification(store.savedTarget.id);
            }}
            onVerifySuccess={() => {
              if (!store.savedTarget) return;
              setSession((prev) => {
                // Imported plans must survive re-verify; only wipe generated plans.
                if (prev.attackPlanSource === "imported" && prev.attackPlan) {
                  return prev;
                }
                const next = {
                  ...prev,
                  attackPlan: null,
                  attackPlanUi: createInitialAttackPlanUi(),
                  attackPlanSource: null,
                };
                saveWizardSession(next);
                return next;
              });
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
              consoleResetKey={consoleResetKey}
              onViewResult={goToResultsStep}
              onClose={handleCancel}
              onRetryFailedCategories={() => {
                void handleRetryFailedCategories();
              }}
              retryFailedPending={startingScan}
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
            onRetryScan={() => {
              updateSession({ currentStep: 4 });
            }}
            onStartAttack={() => {
              void handleRetryScan();
            }}
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
        scanStatus={submittedStatus?.status}
        onStepChange={handleStepChange}
      />

      <Card className="wizard-panel">
        <header className="wizard-panel__header">
          <p className="wizard-panel__step-label text-muted">
            Step {session.currentStep} of 6 · {stepDef.label}
          </p>
          <div className="wizard-panel__title-row">
            <h2 className="wizard-panel__title">{stepDef.title}</h2>
            {step4PlannerBadge ? (
              step4PlannerBadge.label === "AI Planned" ? (
                <YazgBadge />
              ) : (
                <Badge variant={step4PlannerBadge.variant}>{step4PlannerBadge.label}</Badge>
              )
            ) : null}
          </div>
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
                    attackPlanSource: null,
                  });
                  void runAttackPlanner(store.savedTarget!.id, { replan: true });
                }}
              >
                <span className="btn__content">
                  <IconAi className="btn__icon" aria-hidden />
                  {plannerGenerating && plannerReplanning ? "Re-planning…" : "Re-plan"}
                </span>
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
            {showDone && (
              <Button variant="primary" onClick={handleCancel}>
                Done
              </Button>
            )}
            {showFooterNext && (
              <Button
                variant="primary"
                disabled={nextDisabled || persistingTarget || startingScan}
                onClick={() => void handleNext()}
              >
                {importFooterLabel()}
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
