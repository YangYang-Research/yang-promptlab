import { useState } from "react";

import { Button } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  finishAuthRecordSession,
  startAuthRecordSession,
  type AuthRecordFinishDto,
} from "@/shared/ipc/auth";

import type { TargetAuthKind, TargetFormState } from "./targetDescriptor";

type RecordPhase = "idle" | "recording" | "saving" | "verified" | "error";

type PlaywrightRecordPanelProps = {
  form: TargetFormState;
  authKind: Extract<TargetAuthKind, "username_password" | "sso">;
  onChange: (patch: Partial<TargetFormState>) => void;
  startLabel: string;
};

const USER_PASS_STEPS = [
  "Launch Browser",
  "User login manually",
  "Click Finish Recording",
  "AISec saves storageState",
  "Authentication Verified",
] as const;

const SSO_STEPS = [
  "Launch Browser",
  "User signs in",
  "AISec records session",
  "Save browser state",
] as const;

function activeStepIndex(phase: RecordPhase, stepCount: number): number {
  switch (phase) {
    case "idle":
      return -1;
    case "recording":
      return 1;
    case "saving":
      return stepCount - 2;
    case "verified":
      return stepCount - 1;
    default:
      return -1;
  }
}

export function PlaywrightRecordPanel({
  form,
  authKind,
  onChange,
  startLabel,
}: PlaywrightRecordPanelProps) {
  const [phase, setPhase] = useState<RecordPhase>(
    form.browserSessionReady ? "verified" : "idle",
  );
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const steps = authKind === "username_password" ? USER_PASS_STEPS : SSO_STEPS;
  const currentStep = activeStepIndex(phase, steps.length);

  async function handleStart() {
    setError(null);

    if (!form.url.trim()) {
      setError("Enter a Target URL before recording.");
      setPhase("error");
      return;
    }

    if (authKind === "username_password") {
      if (!form.loginUsername.trim()) {
        setError("Username is required.");
        setPhase("error");
        return;
      }
      if (!form.loginPassword) {
        setError("Password is required.");
        setPhase("error");
        return;
      }
    }

    setBusy(true);
    try {
      await startAuthRecordSession({
        loginUrl: form.url.trim(),
        method: authKind === "username_password" ? "username_password" : "oauth",
        config:
          authKind === "username_password"
            ? {
                type: "username_password",
                username: form.loginUsername.trim(),
                password: form.loginPassword,
              }
            : { type: "oauth" },
      });
      setPhase("recording");
    } catch (err) {
      setError(toAppError(err).message);
      setPhase("error");
    } finally {
      setBusy(false);
    }
  }

  async function handleFinish() {
    setError(null);
    setPhase("saving");
    setBusy(true);
    try {
      const result: AuthRecordFinishDto = await finishAuthRecordSession();
      onChange({
        browserSessionReady: true,
        browserSessionId: result.sessionId,
      });
      setPhase("verified");
    } catch (err) {
      setError(toAppError(err).message);
      setPhase("recording");
    } finally {
      setBusy(false);
    }
  }

  function handleReset() {
    setError(null);
    setPhase("idle");
    onChange({ browserSessionReady: false, browserSessionId: null });
  }

  return (
    <div className="wizard-auth-record">
      <ol className="wizard-auth-record__steps">
        {steps.map((label, index) => {
          const done = currentStep > index || phase === "verified";
          const active = currentStep === index;
          return (
            <li
              key={label}
              className={[
                "wizard-auth-record__step",
                done ? "wizard-auth-record__step--done" : "",
                active ? "wizard-auth-record__step--active" : "",
              ]
                .filter(Boolean)
                .join(" ")}
            >
              <span className="wizard-auth-record__step-marker" aria-hidden>
                {done ? "✓" : index + 1}
              </span>
              <span>{label}</span>
            </li>
          );
        })}
      </ol>

      <div className="wizard-auth-record__actions">
        {phase === "idle" || phase === "error" ? (
          <Button type="button" variant="secondary" disabled={busy} onClick={() => void handleStart()}>
            {busy ? "Launching…" : startLabel}
          </Button>
        ) : null}

        {phase === "recording" ? (
          <Button type="button" variant="primary" disabled={busy} onClick={() => void handleFinish()}>
            {busy ? "Saving…" : "Finish Recording"}
          </Button>
        ) : null}

        {phase === "verified" ? (
          <p className="wizard-auth-record__verified">Authentication Verified</p>
        ) : null}

        {(phase === "recording" || phase === "verified" || phase === "error") && !busy ? (
          <Button type="button" variant="ghost" onClick={handleReset}>
            Record again
          </Button>
        ) : null}
      </div>

      {phase === "saving" && (
        <p className="text-muted text-sm">Saving browser session…</p>
      )}

      {error && <p className="text-danger text-sm">{error}</p>}
    </div>
  );
}
