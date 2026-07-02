import {
  verificationPipelineLiveDetail,
  type VerificationPipelinePhase,
} from "../verificationPipeline";
import { VerificationSegmentedProgress } from "./VerificationSegmentedProgress";

type VerificationProgressPipelineProps = {
  phase: VerificationPipelinePhase;
  resultMessage?: string | null;
};

export function VerificationProgressPipeline({
  phase,
  resultMessage,
}: VerificationProgressPipelineProps) {
  if (phase === "idle") return null;

  const liveDetail = verificationPipelineLiveDetail(phase, resultMessage);

  return (
    <div className="auth-verification-step__pipeline">
      <VerificationSegmentedProgress phase={phase} />
      {liveDetail && (
        <p className="wizard-execution-pipeline__live text-sm text-muted">{liveDetail}</p>
      )}
    </div>
  );
}
