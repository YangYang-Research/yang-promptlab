import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { mapProjects } from "@/app/store/mappers";
import { Button, Card, PageHeader } from "@/shared/components";
import { getProject } from "@/shared/ipc";
import type { Project, Target } from "@/shared/types";

import type { AttackPlanConfig } from "./attackProfiles";
import { AttackPlanStep } from "./steps/AttackPlanStep";
import { DiscoveryStep, type DiscoverySelection } from "./steps/DiscoveryStep";
import { ProjectStep } from "./steps/ProjectStep";
import { ResultsStep } from "./steps/ResultsStep";
import { SubmitStep } from "./steps/SubmitStep";
import { TargetStep } from "./steps/TargetStep";
import { WizardStepper } from "./WizardStepper";
import {
  canNavigateToStep,
  canProceedFromStep,
  getWizardStep,
  type WizardDraft,
  type WizardStepId,
} from "./wizardSteps";

export function ScanWizardPage() {
  const [searchParams] = useSearchParams();
  const lockedProjectId = searchParams.get("projectId")?.trim() ?? "";
  const { projects, loading, error, dispatch, actions } = useAppStore();
  const [resolvedProject, setResolvedProject] = useState<Project | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [savedTarget, setSavedTarget] = useState<Target | null>(null);
  const [discoverySelection, setDiscoverySelection] = useState<DiscoverySelection>({
    scanId: null,
    selectedCount: 0,
    selectedEndpointIds: [],
  });
  const [attackPlan, setAttackPlan] = useState<AttackPlanConfig | null>(null);
  const [submittedScanId, setSubmittedScanId] = useState<string | null>(null);
  const [currentStep, setCurrentStep] = useState<WizardStepId>(1);

  const storeProject = lockedProjectId
    ? projects.find((project) => project.id === lockedProjectId)
    : null;
  const lockedProject = storeProject ?? resolvedProject;
  const activeProjectId = lockedProjectId || selectedProjectId;

  const draft: WizardDraft = useMemo(
    () => ({
      projectId: activeProjectId,
      target: savedTarget,
      discovery: discoverySelection,
      attackPlan,
      submittedScanId,
    }),
    [activeProjectId, savedTarget, discoverySelection, attackPlan, submittedScanId],
  );

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
    if (!submittedScanId || currentStep < 5) return;
    const timer = window.setInterval(() => void actions.refresh(), 3000);
    return () => window.clearInterval(timer);
  }, [submittedScanId, currentStep, actions]);

  const stepDef = getWizardStep(currentStep);
  const showNext = currentStep < 6;
  const nextDisabled = !canProceedFromStep(currentStep, draft);

  function handleStepChange(step: WizardStepId) {
    if (canNavigateToStep(step, draft)) {
      setCurrentStep(step);
    }
  }

  function handleNext() {
    if (currentStep >= 6 || !canProceedFromStep(currentStep, draft)) return;
    setCurrentStep((currentStep + 1) as WizardStepId);
  }

  function handleBack() {
    if (currentStep > 1) {
      setCurrentStep((currentStep - 1) as WizardStepId);
    }
  }

  function resetWizard() {
    setCurrentStep(1);
    setSavedTarget(null);
    setDiscoverySelection({ scanId: null, selectedCount: 0, selectedEndpointIds: [] });
    setAttackPlan(null);
    setSubmittedScanId(null);
    if (!lockedProjectId) {
      setSelectedProjectId("");
      dispatch({ type: "SET_SELECTED_PROJECT", projectId: null });
    }
  }

  function handleSubmitted(scanId: string) {
    setSubmittedScanId(scanId);
  }

  function renderStepBody() {
    switch (currentStep) {
      case 1:
        return (
          <ProjectStep
            lockedProjectId={lockedProjectId}
            lockedProject={lockedProject}
            resolveError={resolveError}
            loading={loading}
            projects={projects}
            selectedProjectId={selectedProjectId}
            onSelectProject={(projectId) => {
              setSelectedProjectId(projectId);
              dispatch({ type: "SET_SELECTED_PROJECT", projectId: projectId || null });
            }}
          />
        );
      case 2:
        return activeProjectId ? (
          <TargetStep projectId={activeProjectId} onTargetSaved={setSavedTarget} />
        ) : (
          <p className="text-muted">Select a project in step 1 to configure the target.</p>
        );
      case 3:
        return savedTarget ? (
          <DiscoveryStep target={savedTarget} onSelectionChange={setDiscoverySelection} />
        ) : (
          <p className="text-muted">Save a target in step 2 to run discovery.</p>
        );
      case 4:
        return discoverySelection.selectedCount > 0 ? (
          <AttackPlanStep
            selectedEndpointCount={discoverySelection.selectedCount}
            onPlanChange={setAttackPlan}
          />
        ) : (
          <p className="text-muted">Select at least one endpoint in step 3 to plan attacks.</p>
        );
      case 5:
        return activeProjectId &&
          savedTarget &&
          attackPlan &&
          attackPlan.categories.length > 0 &&
          discoverySelection.selectedEndpointIds.length > 0 ? (
          <SubmitStep
            projectId={activeProjectId}
            target={savedTarget}
            endpointIds={discoverySelection.selectedEndpointIds}
            attackPlan={attackPlan}
            submittedScanId={submittedScanId}
            onSubmitted={handleSubmitted}
            onCreateAnother={resetWizard}
          />
        ) : (
          <p className="text-muted">Complete steps 1–4 before submitting the scan.</p>
        );
      case 6:
        return submittedScanId && activeProjectId ? (
          <ResultsStep projectId={activeProjectId} scanId={submittedScanId} />
        ) : (
          <p className="text-muted">Submit a scan in step 5 to review results.</p>
        );
      default:
        return null;
    }
  }

  return (
    <div className="page">
      <PageHeader title="New Scan" description="Configure a new security scan" />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <WizardStepper currentStep={currentStep} draft={draft} onStepChange={handleStepChange} />

      <Card className="wizard-panel">
        <header className="wizard-panel__header">
          <p className="wizard-panel__step-label text-muted">
            Step {currentStep} of 6 · {stepDef.label}
          </p>
          <h2 className="wizard-panel__title">{stepDef.title}</h2>
          <p className="wizard-panel__hint text-muted">{stepDef.hint}</p>
        </header>

        <div className="wizard-panel__body">{renderStepBody()}</div>

        <footer className="wizard-panel__footer">
          <Button variant="ghost" disabled={currentStep === 1} onClick={handleBack}>
            Back
          </Button>
          {showNext && (
            <Button variant="primary" disabled={nextDisabled} onClick={handleNext}>
              Next
            </Button>
          )}
        </footer>
      </Card>
    </div>
  );
}
