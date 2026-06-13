import type { WizardDraft, WizardStepId } from "./wizardSteps";
import { WIZARD_STEPS, canNavigateToStep } from "./wizardSteps";
import { isTargetFormValid } from "./wizardState";

type WizardStepperProps = {
  currentStep: WizardStepId;
  draft: WizardDraft;
  onStepChange: (step: WizardStepId) => void;
};

export function WizardStepper({ currentStep, draft, onStepChange }: WizardStepperProps) {
  return (
    <nav className="wizard-stepper" aria-label="Scan wizard progress">
      <ol className="wizard-stepper__list">
        {WIZARD_STEPS.map((step, index) => {
          const reachable = canNavigateToStep(step.id, draft);
          const active = step.id === currentStep;
          const complete = step.id < currentStep || (step.id < 6 && isStepDone(step.id, draft));

          return (
            <li
              key={step.id}
              className={`wizard-stepper__item${active ? " wizard-stepper__item--active" : ""}${complete ? " wizard-stepper__item--complete" : ""}`}
            >
              <button
                type="button"
                className="wizard-stepper__button"
                disabled={!reachable && !active}
                aria-current={active ? "step" : undefined}
                onClick={() => reachable && onStepChange(step.id)}
              >
                <span className="wizard-stepper__index">{step.id}</span>
                <span className="wizard-stepper__label">{step.label}</span>
              </button>
              {index < WIZARD_STEPS.length - 1 && (
                <span className="wizard-stepper__connector" aria-hidden="true" />
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

function isStepDone(step: WizardStepId, draft: WizardDraft): boolean {
  if (step === 1) return draft.projectId.length > 0;
  if (step === 2) return draft.target !== null && isTargetFormValid(draft.targetForm);
  if (step === 3) return draft.discoveryCompleted && draft.discovery.selectedCount > 0;
  if (step === 4) return (draft.attackPlan?.categories.length ?? 0) > 0;
  if (step === 5) return draft.submittedScanId !== null;
  return false;
}
