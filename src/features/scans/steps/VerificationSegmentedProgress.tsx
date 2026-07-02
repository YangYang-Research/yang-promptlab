import { useEffect, useState } from "react";

import { IconCheck, IconX } from "@/shared/components/Icons";

import {
  aiSegmentLabel,
  authSegmentLabel,
  resolveVerificationSegmentProgress,
  type VerificationPipelinePhase,
  type VerificationSegmentState,
} from "../verificationPipeline";

type VerificationSegmentedProgressProps = {
  phase: VerificationPipelinePhase;
};

const ACTIVE_PROGRESS_CAP = 92;

function useAnimatedSegmentProgress(state: VerificationSegmentState): number {
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    if (state === "pending") {
      setProgress(0);
      return;
    }

    if (state === "done" || state === "failed") {
      setProgress(100);
      return;
    }

    setProgress(0);
    let rafId = 0;
    const startedAt = performance.now();

    const tick = (now: number) => {
      const elapsed = now - startedAt;
      const next = Math.min(
        ACTIVE_PROGRESS_CAP,
        ACTIVE_PROGRESS_CAP * (1 - Math.exp(-elapsed / 2800)),
      );
      setProgress(next);
      rafId = requestAnimationFrame(tick);
    };

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [state]);

  return progress;
}

type SegmentHalfProps = {
  tone: "auth" | "ai";
  state: VerificationSegmentState;
  progress: number;
  label: string;
};

function SegmentHalf({ tone, state, progress, label }: SegmentHalfProps) {
  return (
    <div
      className={[
        "verification-segments__half",
        `verification-segments__half--${tone}`,
        state !== "pending" ? `verification-segments__half--${state}` : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div
        className="verification-segments__track-inner"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(progress)}
        aria-label={label}
      >
        <div className="verification-segments__fill" style={{ width: `${progress}%` }} />
      </div>
    </div>
  );
}

type SegmentLabelProps = {
  label: string;
  tone: "auth" | "ai";
  state: VerificationSegmentState;
};

function SegmentLabel({ label, tone, state }: SegmentLabelProps) {
  return (
    <div
      className={[
        "verification-segments__label-cell",
        `verification-segments__label-cell--${tone}`,
        state !== "pending" ? `verification-segments__label-cell--${state}` : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <span className="verification-segments__part-label">{label}</span>
      {state === "done" && (
        <span className="verification-segments__status-icon verification-segments__status-icon--success" aria-hidden>
          <IconCheck />
        </span>
      )}
      {state === "failed" && (
        <span className="verification-segments__status-icon verification-segments__status-icon--failed" aria-hidden>
          <IconX />
        </span>
      )}
    </div>
  );
}

export function VerificationSegmentedProgress({ phase }: VerificationSegmentedProgressProps) {
  const segments = resolveVerificationSegmentProgress(phase);
  const authProgress = useAnimatedSegmentProgress(segments.auth);
  const aiProgress = useAnimatedSegmentProgress(segments.ai);

  return (
    <div className="verification-segments">
      <div className="verification-segments__header">
        <span className="verification-segments__title">Verification progress</span>
      </div>

      <div
        className="verification-segments__bar-row"
        role="group"
        aria-label="Verification progress in two stages"
      >
        <SegmentHalf
          tone="auth"
          state={segments.auth}
          progress={authProgress}
          label={authSegmentLabel(segments.auth)}
        />
        <span className="verification-segments__divider" aria-hidden>
          ·
        </span>
        <SegmentHalf
          tone="ai"
          state={segments.ai}
          progress={aiProgress}
          label={aiSegmentLabel(segments.ai)}
        />
      </div>

      <div className="verification-segments__label-row">
        <SegmentLabel
          label={authSegmentLabel(segments.auth)}
          tone="auth"
          state={segments.auth}
        />
        <span className="verification-segments__divider verification-segments__divider--spacer" aria-hidden>
          ·
        </span>
        <SegmentLabel label={aiSegmentLabel(segments.ai)} tone="ai" state={segments.ai} />
      </div>
    </div>
  );
}
