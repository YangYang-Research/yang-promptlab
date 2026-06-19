import type { TargetFormState } from "../targetDescriptor";
import { TargetFormFields } from "../TargetFormFields";

type TargetStepProps = {
  form: TargetFormState;
  onChange: (patch: Partial<TargetFormState>) => void;
  error: string | null;
};

export function TargetStep({ form, onChange, error }: TargetStepProps) {
  return (
    <div className="wizard-step">
      <TargetFormFields form={form} onChange={onChange} error={error} />
    </div>
  );
}
