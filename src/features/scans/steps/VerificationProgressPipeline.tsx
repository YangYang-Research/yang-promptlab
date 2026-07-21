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
  const isFailure = phase === "failed_auth" || phase === "failed_ai";

  return (
    <div className="auth-verification-step__pipeline">
      <VerificationSegmentedProgress phase={phase} />
      {liveDetail && (
        <p
          className={[
            "wizard-execution-pipeline__live",
            "text-sm",
            isFailure ? "text-danger auth-verification-step__error" : "text-muted",
          ].join(" ")}
          role={isFailure ? "alert" : undefined}
        >
          {liveDetail}
        </p>
      )}
    </div>
  );
}
