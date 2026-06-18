import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { mapProjects } from "@/app/store/mappers";
import { Button, Card, PageHeader } from "@/shared/components";
import { getProject, startScan } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { Project, Target } from "@/shared/types";

import { AttackPlanStep } from "./steps/AttackPlanStep";
import { DiscoveryStep } from "./steps/DiscoveryStep";
import { ProjectStep } from "./steps/ProjectStep";
import { ResultsStep } from "./steps/ResultsStep";
import { SubmitStep } from "./steps/SubmitStep";
import { TargetStep } from "./steps/TargetStep";
import {
  buildTargetDescriptor,
  deriveTargetName,
  targetFormFingerprint,
  validateTargetStep,
  type TargetFormState,
} from "./targetDescriptor";
import { WizardStepper } from "./WizardStepper";
import {
  buildWizardStore,
  clearWizardSession,
  loadWizardSession,
  resetSessionForNewScan,
  saveWizardSession,
  shouldPersistTarget,
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
  const { projects, targets, loading, error, dispatch, actions } = useAppStore();
  const { notify } = useToast();

  const [session, setSession] = useState<ScanWizardSession>(() =>
    loadWizardSession(lockedProjectId),
  );
  const [resolvedProject, setResolvedProject] = useState<Project | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [targetStepError, setTargetStepError] = useState<string | null>(null);
  const [scanSubmitError, setScanSubmitError] = useState<string | null>(null);
  const [persistingTarget, setPersistingTarget] = useState(false);
  const [startingScan, setStartingScan] = useState(false);

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
      discovery: store.discoverySelection,
      discoveryCompleted: session.discovery.completed,
      attackPlan: session.attackPlan,
      submittedScanId: session.submittedScanId,
    }),
    [
      activeProjectId,
      session.targetForm,
      store.savedTarget,
      store.discoverySelection,
      session.discovery.completed,
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
    if (!session.submittedScanId || session.currentStep < 5) return;
    const timer = window.setInterval(() => void actions.refresh(), 3000);
    return () => window.clearInterval(timer);
  }, [session.submittedScanId, session.currentStep, actions]);

  const stepDef = getWizardStep(session.currentStep);
  const showFooterNext =
    session.currentStep < 5 || (session.currentStep === 5 && session.submittedScanId !== null);
  const showStartScan = session.currentStep === 5 && session.submittedScanId === null;
  const showFooterDone = session.currentStep === 6;
  const hideBack =
    session.currentStep === 6 ||
    (session.currentStep === 5 && session.submittedScanId !== null);
  const nextDisabled = !canProceedFromStep(session.currentStep, draft);
  const startScanDisabled = !canStartScan(draft) || startingScan;

  function patchTargetForm(patch: Partial<TargetFormState>) {
    setTargetStepError(null);
    setSession((prev) => {
      const next = { ...prev, targetForm: { ...prev.targetForm, ...patch } };
      saveWizardSession(next);
      return next;
    });
  }

  async function persistTargetIfNeeded(): Promise<Target | null> {
    const validationError = validateTargetStep(session.targetForm);
    if (validationError) {
      setTargetStepError(validationError);
      return null;
    }

    const fingerprint = targetFormFingerprint(session.targetForm);
    if (
      store.savedTarget &&
      !shouldPersistTarget(session.targetForm, store.savedTarget, session.savedTargetFingerprint)
    ) {
      // Reuse existing target when form unchanged.
      return store.savedTarget;
    }

    setPersistingTarget(true);
    setTargetStepError(null);
    try {
      const descriptor = buildTargetDescriptor(session.targetForm);
      const name = deriveTargetName(session.targetForm.url);
      const target = await actions.createTarget(activeProjectId, name, "web", descriptor);
      updateSession({
        savedTargetId: target.id,
        savedTargetFingerprint: fingerprint,
      });
      notify(`Target "${name}" saved`, "success");
      return target;
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to save target";
      setTargetStepError(message);
      notify(message, "error");
      return null;
    } finally {
      setPersistingTarget(false);
    }
  }

  function handleStepChange(step: WizardStepId) {
    if (canNavigateToStep(step, draft)) {
      updateSession({ currentStep: step });
    }
  }

  async function handleNext() {
    if (session.currentStep >= 6) return;

    if (session.currentStep === 2) {
      const target = await persistTargetIfNeeded();
      if (!target) return;
      updateSession({ currentStep: 3 });
      return;
    }

    if (!canProceedFromStep(session.currentStep, draft)) return;
    updateSession({ currentStep: (session.currentStep + 1) as WizardStepId });
  }

  function handleBack() {
    if (session.currentStep > 1) {
      updateSession({ currentStep: (session.currentStep - 1) as WizardStepId });
    }
  }

  function handleCancel() {
    navigate("/scans");
  }

  async function handleStartScan() {
    if (!canStartScan(draft) || !store.savedTarget || !session.attackPlan) return;

    setStartingScan(true);
    setScanSubmitError(null);
    try {
      const result = await startScan({
        projectId: activeProjectId,
        targetId: store.savedTarget.id,
        endpointIds: session.discovery.selectedEndpointIds,
        categories: session.attackPlan.categories,
        profile: session.attackPlan.profileId,
        disabledTests: session.attackPlan.disabledTests,
        generatorMode: session.attackPlan.generatorMode,
        agentMode: session.attackPlan.agentMode,
        maxAgentAttempts: session.attackPlan.maxAgentAttempts,
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

  function resetWizard() {
    const fresh = resetSessionForNewScan(lockedProjectId, session.selectedProjectId);
    clearWizardSession();
    saveWizardSession(fresh);
    setSession(fresh);
    setTargetStepError(null);
    setScanSubmitError(null);
    if (!lockedProjectId) {
      dispatch({ type: "SET_SELECTED_PROJECT", projectId: null });
    }
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
          <TargetStep
            form={session.targetForm}
            onChange={patchTargetForm}
            error={targetStepError}
          />
        ) : (
          <p className="text-muted">Select a project in step 1 to configure the target.</p>
        );
      case 3:
        return store.savedTarget ? (
          <DiscoveryStep
            target={store.savedTarget}
            discovery={session.discovery}
            onDiscoveryChange={(patch) =>
              setSession((prev) => {
                const next = { ...prev, discovery: { ...prev.discovery, ...patch } };
                saveWizardSession(next);
                return next;
              })
            }
          />
        ) : (
          <p className="text-muted">Complete step 2 to run discovery.</p>
        );
      case 4:
        return session.discovery.selectedEndpointIds.length > 0 ? (
          <AttackPlanStep
            selectedEndpointCount={session.discovery.selectedEndpointIds.length}
            endpoints={session.discovery.endpoints}
            selectedEndpointIds={session.discovery.selectedEndpointIds}
            planUi={session.attackPlanUi}
            onPlanUiChange={(patch) =>
              setSession((prev) => {
                const next = { ...prev, attackPlanUi: { ...prev.attackPlanUi, ...patch } };
                saveWizardSession(next);
                return next;
              })
            }
            onPlanChange={(plan) => updateSession({ attackPlan: plan })}
          />
        ) : (
          <p className="text-muted">Select at least one endpoint in step 3 to plan attacks.</p>
        );
      case 5:
        return activeProjectId &&
          store.savedTarget &&
          session.attackPlan &&
          session.attackPlan.categories.length > 0 &&
          session.discovery.selectedEndpointIds.length > 0 ? (
          <>
            <SubmitStep
              target={store.savedTarget}
              endpointIds={session.discovery.selectedEndpointIds}
              attackPlan={session.attackPlan}
              submittedScanId={session.submittedScanId}
              onCreateAnother={resetWizard}
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
          <p className="wizard-panel__hint text-muted">{stepDef.hint}</p>
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
    </div>
  );
}
