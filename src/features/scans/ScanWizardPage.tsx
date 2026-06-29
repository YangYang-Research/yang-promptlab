import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { mapProjects, mapTargets } from "@/app/store/mappers";
import { Button, Card, PageHeader, Badge } from "@/shared/components";
import { getProject, startScan, updateTargetDescriptor } from "@/shared/ipc";
import { generateAttackPlanForTarget } from "@/shared/ipc/attackPlanner";
import { getTargetProfile, saveTargetProfile } from "@/shared/ipc/targetProfile";
import { toAppError } from "@/shared/errors";
import { useToast } from "@/shared/notifications";
import type { Project, Target } from "@/shared/types";

import { ImportApiModal } from "./components/ImportApiModal";
import { ReviewAttackPlanStep } from "./steps/ReviewAttackPlanStep";
import { AuthVerificationStep } from "./steps/AuthVerificationStep";
import { ProjectStep } from "./steps/ProjectStep";
import { ResultsStep } from "./steps/ResultsStep";
import { SubmitStep } from "./steps/SubmitStep";
import { TargetProfileStep } from "./steps/TargetProfileStep";
import { mergeScanStatus, useScanStatuses } from "./useScanStatuses";
import {
  deriveTargetNameFromProfile,
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
import { attackPlanFromDto } from "./attackPlan";
import {
  buildWizardStore,
  clearWizardSession,
  createSessionForTargetScan,
  attackPlanUiFromPlan,
  createInitialAttackPlanUi,
  fetchTargetFormForWizard,
  loadTargetDtoForWizard,
  loadWizardSession,
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

export function ScanWizardPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const lockedProjectId = searchParams.get("projectId")?.trim() ?? "";
  const lockedTargetId = searchParams.get("targetId")?.trim() ?? "";
  const requestedStep = searchParams.get("step")?.trim() ?? "";
  const { projects, targets, loading, error, dispatch, actions } = useAppStore();
  const { notify } = useToast();
  const deepLinkApplied = useRef(false);
  const deepLinkTargetId = useRef<string | null>(null);

  const [session, setSession] = useState<ScanWizardSession>(() =>
    loadWizardSession(lockedProjectId),
  );
  const [resolvedProject, setResolvedProject] = useState<Project | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [profileStepError, setProfileStepError] = useState<string | null>(null);
  const [verificationError, setVerificationError] = useState<string | null>(null);
  const [scanSubmitError, setScanSubmitError] = useState<string | null>(null);
  const [persistingTarget, setPersistingTarget] = useState(false);
  const [startingScan, setStartingScan] = useState(false);
  const [plannerGenerating, setPlannerGenerating] = useState(false);
  const [plannerError, setPlannerError] = useState<string | null>(null);
  const [planAdjusting, setPlanAdjusting] = useState(false);
  const [dbVerification, setDbVerification] = useState<VerificationResultForm | null>(null);
  const [dbVerificationLoading, setDbVerificationLoading] = useState(false);
  const [importApiOpen, setImportApiOpen] = useState(false);
  const plannerRunRef = useRef<string | null>(null);

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
      const next = { ...prev, ...patch };
      saveWizardSession(next);
      return next;
    });
  }, []);

  const runAttackPlanner = useCallback(
    async (targetId: string, options?: { autoAdvance?: boolean }) => {
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
            attackPlanUi: attackPlanUiFromPlan(plan),
            currentStep: options?.autoAdvance ? (4 as WizardStepId) : prev.currentStep,
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
    if (
      !store.savedTarget ||
      !session.targetProfile.verification.verified ||
      session.attackPlan
    ) {
      return;
    }
    void runAttackPlanner(store.savedTarget.id);
  }, [
    store.savedTarget,
    session.targetProfile.verification.verified,
    session.attackPlan,
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
    if (deepLinkTargetId.current !== lockedTargetId) {
      deepLinkTargetId.current = lockedTargetId || null;
      deepLinkApplied.current = false;
    }
    if (!lockedTargetId || deepLinkApplied.current) return;

    let cancelled = false;
    const step: WizardStepId = 3;

    void loadTargetDtoForWizard(lockedTargetId)
      .then((dto) => {
        if (cancelled) return;
        const target = mapTargets([dto])[0];
        if (lockedProjectId && target.projectId !== lockedProjectId) {
          notify("Target does not belong to this project", "error");
          return;
        }

        const profileState =
          dto.profile && typeof dto.profile === "object"
            ? profileFromDto(dto.profile as Parameters<typeof profileFromDto>[0])
            : session.targetProfile;
        const targetForm = targetFormFromDescriptor(dto.descriptor, fullProfileUrl(profileState));
        const next = {
          ...createSessionForTargetScan(
            lockedProjectId || target.projectId,
            target,
            dto.descriptor,
            dto.profile,
            step,
          ),
          targetForm,
          savedTargetFingerprint: targetFormFingerprint(targetForm),
        };
        deepLinkApplied.current = true;
        setSession(next);
        saveWizardSession(next);
        dispatch({ type: "SET_SELECTED_PROJECT", projectId: next.selectedProjectId });
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
  }, [lockedTargetId, lockedProjectId, requestedStep, dispatch, notify]);

  useEffect(() => {
    if (!session.submittedScanId || session.currentStep < 5) return;
    const timer = window.setInterval(() => void actions.refresh(), 3000);
    return () => window.clearInterval(timer);
  }, [session.submittedScanId, session.currentStep, actions]);

  const submittedStatuses = useScanStatuses(
    session.submittedScanId ? [session.submittedScanId] : [],
    session.currentStep === 5 && session.submittedScanId !== null,
  );
  const submittedLiveStatus = session.submittedScanId
    ? submittedStatuses.get(session.submittedScanId)
    : undefined;
  const submittedStatus = session.submittedScanId
    ? mergeScanStatus(session.submittedScanId, "running", submittedLiveStatus, 0)
    : null;

  const stepDef = getWizardStep(session.currentStep);
  const showFooterNext =
    session.currentStep < 5 &&
    (session.currentStep !== 3 ||
      (draft.profileVerified && draft.attackPlanGenerated && !plannerGenerating));
  const showStartScan = session.currentStep === 5 && session.submittedScanId === null;
  const showFooterDone = session.currentStep === 6;
  const showViewResult =
    session.currentStep === 5 && submittedStatus?.status === "completed";
  const showRetryScan =
    session.currentStep === 5 &&
    submittedStatus !== null &&
    (submittedStatus.status === "failed" ||
      submittedStatus.status === "stopped" ||
      submittedStatus.status === "cancelled");
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
        const targetForm = await prepareAuthFormForStep3(
          session.targetProfile,
          session.targetForm,
          targetId,
        );
        updateSession({
          currentStep: nextStep,
          targetForm,
          savedTargetFingerprint: targetFormFingerprint(targetForm),
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
      );
      updateSession({
        currentStep: 3,
        targetForm,
        savedTargetFingerprint: targetFormFingerprint(targetForm),
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
      if (!session.attackPlan) {
        const plan = await runAttackPlanner(store.savedTarget.id);
        if (!plan) return;
      }
      updateSession({ currentStep: 4 });
      return;
    }

    if (!canProceedFromStep(session.currentStep, draft)) return;
    updateSession({ currentStep: (session.currentStep + 1) as WizardStepId });
  }

  function handleBack() {
    if (session.currentStep > 1) {
      void navigateToStep((session.currentStep - 1) as WizardStepId);
    }
  }

  function handleCancel() {
    navigate("/scans");
  }

  async function handleStartScan() {
    if (!canStartScan(draft) || !store.savedTarget || !session.attackPlan) return;
    await submitScanJob();
  }

  async function submitScanJob() {
    if (!store.savedTarget || !session.attackPlan) return;

    setStartingScan(true);
    setScanSubmitError(null);
    try {
      const result = await startScan({
        projectId: activeProjectId,
        targetId: store.savedTarget.id,
        categories: session.attackPlan.categories,
        profile: session.attackPlan.profileId,
        disabledTests: session.attackPlan.disabledTests,
        agentMode: session.attackPlan.executionStrategy === "agentic",
        maxAgentAttempts: session.attackPlan.maxAttempts,
      });
      await actions.refresh();
      updateSession({ submittedScanId: result.scan_id });
      notify("Scan started in the background", "success");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to start scan";
      setScanSubmitError(message);
      notify(message, "error");
    } finally {
      setStartingScan(false);
    }
  }

  async function handleRetryScan() {
    if (!store.savedTarget || !session.attackPlan) return;
    await submitScanJob();
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
            verificationConsole={session.verificationConsole}
            onVerificationConsole={(entry) => updateSession({ verificationConsole: entry })}
            error={verificationError}
            onError={setVerificationError}
            onBeforeVerify={persistAuthDescriptor}
            onVerifySettled={() => {
              if (store.savedTarget) void refreshDbVerification(store.savedTarget.id);
            }}
            onVerifySuccess={() => {
              if (!store.savedTarget) return;
              updateSession({ attackPlan: null, attackPlanUi: createInitialAttackPlanUi() });
              plannerRunRef.current = null;
              void runAttackPlanner(store.savedTarget.id, { autoAdvance: true });
            }}
          />
            {plannerGenerating && (
              <p className="text-muted text-sm">Generating attack plan…</p>
            )}
            {plannerError && !plannerGenerating && (
              <p className="text-danger text-sm">{plannerError}</p>
            )}
          </>
        ) : (
          <p className="text-muted">Complete step 2 to configure authentication.</p>
        );
      case 4:
        if (!session.targetProfile.verification.verified) {
          return <p className="text-muted">Waiting for target verification…</p>;
        }
        if (plannerGenerating || (!session.attackPlan && !plannerError)) {
          return <p className="text-muted">Generating attack plan…</p>;
        }
        if (plannerError && !session.attackPlan) {
          return (
            <div>
              <p className="text-danger">{plannerError}</p>
              {store.savedTarget && (
                <Button
                  variant="primary"
                  onClick={() => void runAttackPlanner(store.savedTarget!.id)}
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
          <p className="text-muted">Generating attack plan…</p>
        );
      case 5:
        return activeProjectId &&
          store.savedTarget &&
          session.attackPlan &&
          session.attackPlan.categories.length > 0 &&
          session.targetProfile.verification.verified ? (
          <>
            <SubmitStep
              target={store.savedTarget}
              targetProfile={session.targetProfile}
              attackPlan={session.attackPlan}
              submittedScanId={session.submittedScanId}
              onViewResult={() => updateSession({ currentStep: 6 })}
              onRetryScan={() => void handleRetryScan()}
            />
            {scanSubmitError && <p className="text-danger">{scanSubmitError}</p>}
          </>
        ) : (
          <p className="text-muted">Complete steps 1–4 before submitting the scan.</p>
        );
      case 6:
        return session.submittedScanId && activeProjectId ? (
          <ResultsStep
            projectId={activeProjectId}
            scanId={session.submittedScanId}
            onDone={() => {
              clearWizardSession();
              navigate("/");
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
        title="New Scan"
        description="Configure a new security scan"
        actions={
          <Button variant="danger" onClick={handleCancel}>
            Cancel
          </Button>
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
                {startingScan ? "Starting scan…" : "Start Scan"}
              </Button>
            )}
            {showViewResult && (
              <Button variant="primary" onClick={() => updateSession({ currentStep: 6 })}>
                View Result
              </Button>
            )}
            {showRetryScan && (
              <Button
                variant="primary"
                disabled={startingScan}
                onClick={() => void handleRetryScan()}
              >
                {startingScan ? "Retrying…" : "Retry Scan"}
              </Button>
            )}
            {showFooterNext && (
              <Button
                variant="primary"
                disabled={nextDisabled || persistingTarget}
                onClick={() => void handleNext()}
              >
                {persistingTarget ? "Saving target…" : "Next"}
              </Button>
            )}
            {showFooterDone && (
              <Button
                variant="primary"
                onClick={() => {
                  clearWizardSession();
                  navigate("/");
                }}
              >
                Done
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
