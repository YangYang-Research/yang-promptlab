import type { WizardDraft, WizardStepId } from "./wizardSteps";
import { WIZARD_STEPS, canNavigateToStep, isStepComplete } from "./wizardSteps";

type WizardStepperProps = {
  currentStep: WizardStepId;
  draft: WizardDraft;
  scanStatus?: string | null;
  onStepChange: (step: WizardStepId) => void;
};

export function WizardStepper({
  currentStep,
  draft,
  scanStatus,
  onStepChange,
}: WizardStepperProps) {
  return (
    <nav className="wizard-stepper" aria-label="Scan wizard progress">
      <ol className="wizard-stepper__list">
        {WIZARD_STEPS.map((step, index) => {
          const reachable = canNavigateToStep(step.id, draft, { scanStatus });
          const active = step.id === currentStep;
          const complete = isStepComplete(step.id, draft) && step.id < currentStep;

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
