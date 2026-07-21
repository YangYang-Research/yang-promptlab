import { useEffect, useRef, useState } from "react";

import { Button } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import {
  verifyTargetProfileAi,
  verifyTargetProfileConnect,
} from "@/shared/ipc/targetProfile";
import { useToast } from "@/shared/notifications";
import { assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";

import { TargetFormFields } from "../TargetFormFields";
import { VerificationConsole } from "../components/VerificationConsole";
import type { VerificationPipelinePhase } from "../verificationPipeline";
import { VerificationProgressPipeline } from "./VerificationProgressPipeline";
import {
  AUTH_METHOD_OPTIONS,
  buildTargetDescriptor,
  inferAuthFromProfileHeaders,
  targetFormNeedsSecretHydration,
  type TargetFormState,
} from "../targetDescriptor";
import {
  fullProfileUrl,
  profileFromDto,
  profileToPayload,
  type TargetProfileFormState,
} from "../targetProfile";
import {
  buildVerificationRequestPreview,
  authHeadersFromForm,
  CONNECT_PROBE_PROMPT,
  VERIFY_PROMPT,
} from "../verificationRequest";
import {
  appendVerificationLogLine,
  appendVerificationLogLines,
  formatAuthenticationLogLines,
  formatAiValidationLogLine,
  formatErrorLogLine,
  formatResponseLogLine,
  formatSendRequestLogLine,
  VERIFICATION_LOG_START_AI,
  VERIFICATION_LOG_START_AUTH,
  type VerificationLogLine,
} from "../verificationLog";
import {
  IMPORT_VERIFY_MAX_ATTEMPTS,
  IMPORT_VERIFY_RETRY_DELAY_MS,
  sleep,
} from "../importHarness";
import { logWizardEvent } from "../wizardLiveLog";

type AuthVerificationStepProps = {
  targetId: string;
  profile: TargetProfileFormState;
  onProfileChange: (patch: Partial<TargetProfileFormState>) => void;
  authForm: TargetFormState;
  onAuthChange: (patch: Partial<TargetFormState>) => void;
  verificationLog: VerificationLogLine[];
  onVerificationLog: (lines: VerificationLogLine[]) => void;
  onError: (message: string | null) => void;
  error?: string | null;
  onBeforeVerify?: () => Promise<boolean>;
  onVerifySuccess?: () => void;
  onVerifySettled?: () => void;
  /** When true, start Verification automatically and retry on harness failures. */
  autoVerify?: boolean;
  autoVerifyMaxAttempts?: number;
  onAutoVerifyAttempt?: (attempt: number, maxAttempts: number) => void;
  onAutoVerifyComplete?: (result: {
    ok: boolean;
    attempts: number;
    message: string | null;
  }) => void;
};

function authMethodLabel(kind: TargetFormState["authKind"]): string {
  return AUTH_METHOD_OPTIONS.find((option) => option.value === kind)?.label ?? kind;
}

function phaseFromVerification(
  verification: TargetProfileFormState["verification"],
): VerificationPipelinePhase {
  if (verification.verified) return "done";
  if (verification.status === "failed" || verification.errorMessage) {
    return "failed_ai";
  }
  return "idle";
}

function messageFromVerification(
  verification: TargetProfileFormState["verification"],
): string | null {
  if (verification.verified) {
    return verification.status?.trim() || "Endpoint verified";
  }
  return verification.errorMessage;
}

/** Fingerprint of inputs that affect connectivity/auth probes. */
function connectProbeKey(profile: TargetProfileFormState, authForm: TargetFormState): string {
  return JSON.stringify({
    targetUrl: fullProfileUrl(profile),
    method: profile.method,
    headersJson: profile.headersJson,
    requestTemplate: profile.requestTemplate,
    promptPlaceholder: profile.promptPlaceholder,
    authKind: authForm.authKind,
    loginUrl: authForm.loginUrl,
    loginUsername: authForm.loginUsername,
    loginPassword: authForm.loginPassword,
    basicUsername: authForm.basicUsername,
    basicPassword: authForm.basicPassword,
    apiKeyHeaderName: authForm.apiKeyHeaderName,
    apiKeyValue: authForm.apiKeyValue,
    apiKeyPrefix: authForm.apiKeyPrefix,
    jwtToken: authForm.jwtToken,
    jwtHeaderName: authForm.jwtHeaderName,
    jwtPrefix: authForm.jwtPrefix,
    browserSessionId: authForm.browserSessionId,
  });
}

export function AuthVerificationStep({
  targetId,
  profile,
  onProfileChange,
  authForm,
  onAuthChange,
  verificationLog,
  onVerificationLog,
  onError,
  error = null,
  onBeforeVerify,
  onVerifySuccess,
  onVerifySettled,
  autoVerify = false,
  autoVerifyMaxAttempts = IMPORT_VERIFY_MAX_ATTEMPTS,
  onAutoVerifyAttempt,
  onAutoVerifyComplete,
}: AuthVerificationStepProps) {
  const { notify } = useToast();
  const [verifying, setVerifying] = useState(false);
  const [verifyPhase, setVerifyPhase] = useState<VerificationPipelinePhase>(() =>
    phaseFromVerification(profile.verification),
  );
  const [verifyResultMessage, setVerifyResultMessage] = useState<string | null>(() =>
    messageFromVerification(profile.verification),
  );
  const [authDetectedFromProfile, setAuthDetectedFromProfile] = useState(false);
  const authFormRef = useRef(authForm);
  authFormRef.current = authForm;
  const onAuthChangeRef = useRef(onAuthChange);
  onAuthChangeRef.current = onAuthChange;
  /** Once the user picks an auth method (including None), stop auto-overriding from headers. */
  const authKindChosenByUserRef = useRef(false);
  /** Step 1 passed for this probe key — retry after Step 2 failure skips Step 1. */
  const step1PassedRef = useRef<{ key: string } | null>(null);
  const verifyPhaseRef = useRef(verifyPhase);
  verifyPhaseRef.current = verifyPhase;
  const profileRef = useRef(profile);
  profileRef.current = profile;
  const verificationLogRef = useRef(verificationLog);
  verificationLogRef.current = verificationLog;

  // Restore pipeline UI when returning to step 3 with a persisted verification result.
  useEffect(() => {
    if (verifying) return;
    if (!profile.verification.verified) {
      setVerifyPhase((prev) =>
        prev === "done" ? phaseFromVerification(profile.verification) : prev,
      );
      return;
    }
    setVerifyPhase("done");
    setVerifyResultMessage(messageFromVerification(profile.verification));
  }, [
    verifying,
    profile.verification.verified,
    profile.verification.status,
    profile.verification.errorMessage,
  ]);

  useEffect(() => {
    const current = authFormRef.current;
    const inferred = inferAuthFromProfileHeaders(profile, current);

    if (current.authKind === "none") {
      // Respect an explicit None selection — only auto-detect once before the user picks.
      if (
        !authKindChosenByUserRef.current &&
        inferred.authKind &&
        inferred.authKind !== "none"
      ) {
        setAuthDetectedFromProfile(true);
        onAuthChangeRef.current(inferred);
      } else if (inferred.url && inferred.url !== current.url) {
        onAuthChangeRef.current({ url: inferred.url });
      }
      return;
    }

    if (targetFormNeedsSecretHydration(current)) {
      onAuthChangeRef.current(inferred);
      return;
    }

    if (inferred.url && inferred.url !== current.url) {
      onAuthChangeRef.current({ url: inferred.url });
    }
  }, [profile.headersJson, profile.baseUrl, profile.path, targetId]);

  useEffect(() => {
    const key = connectProbeKey(profile, authForm);
    if (step1PassedRef.current && step1PassedRef.current.key !== key) {
      step1PassedRef.current = null;
    }
  }, [profile, authForm]);

  async function runVerifyOnce(options?: {
    quietToasts?: boolean;
  }): Promise<{ ok: boolean; message: string }> {
    const quietToasts = options?.quietToasts ?? false;
    const currentProfile = profileRef.current;
    const connectPreview = buildVerificationRequestPreview(currentProfile, authFormRef.current, {
      prompt: CONNECT_PROBE_PROMPT,
    });
    const capabilityPreview = buildVerificationRequestPreview(
      currentProfile,
      authFormRef.current,
      { prompt: VERIFY_PROMPT },
    );
    const probeKey = connectProbeKey(currentProfile, authFormRef.current);
    const skipStep1 =
      verifyPhaseRef.current === "failed_ai" && step1PassedRef.current?.key === probeKey;

    setVerifying(true);
    let activePhase: VerificationPipelinePhase = skipStep1 ? "ai" : "auth";
    setVerifyPhase(activePhase);
    setVerifyResultMessage(null);
    onError(null);

    logWizardEvent({
      category: "authentication",
      activityName: skipStep1 ? "verify_ai_start" : "verify_auth_start",
      message: skipStep1
        ? "Starting endpoint analysis (Yazg)"
        : "Starting connectivity / authentication probe",
      component: "AuthVerificationStep",
      attributes: { targetId, skipStep1, quietToasts },
    });

    let log: VerificationLogLine[] = skipStep1 ? [...verificationLogRef.current] : [];
    const publishLog = () => onVerificationLog([...log]);
    const append = (message: string) => {
      log = appendVerificationLogLine(log, message);
      publishLog();
    };
    const appendMany = (messages: string[]) => {
      log = appendVerificationLogLines(log, messages);
      publishLog();
    };

    const profileUrl = fullProfileUrl(currentProfile);
    const descriptor = buildTargetDescriptor({
      ...authFormRef.current,
      url: profileUrl,
    }) as { auth?: Record<string, unknown> };
    const verifyOptions = {
      auth: descriptor.auth ?? null,
      authHeaders: authHeadersFromForm(authFormRef.current),
    };

    if (!skipStep1) {
      appendMany([
        VERIFICATION_LOG_START_AUTH,
        ...formatAuthenticationLogLines(connectPreview.headers),
        formatSendRequestLogLine(connectPreview.requestLog, 1),
      ]);
    } else {
      append("Retrying Step 2 — Analyze Endpoint (skipping Step 1 connectivity check)");
    }

    try {
      if (onBeforeVerify) {
        const ready = await onBeforeVerify();
        if (!ready) {
          setVerifyPhase("failed_auth");
          activePhase = "failed_auth";
          const message = "Authentication was not saved — fix errors above and retry.";
          setVerifyResultMessage(message);
          onError(message);
          append(formatErrorLogLine(message));
          return { ok: false, message };
        }
      }

      if (!skipStep1) {
        const connect = await verifyTargetProfileConnect(
          targetId,
          profileToPayload(currentProfile),
          verifyOptions,
        );
        append(formatResponseLogLine(connect.console, "connectivity"));

        if (!connect.success) {
          step1PassedRef.current = null;
          setVerifyPhase("failed_auth");
          activePhase = "failed_auth";
          setVerifyResultMessage(connect.message);
          onError(connect.message);
          if (!quietToasts) notify(connect.message, "error");
          onProfileChange({
            verification: {
              ...currentProfile.verification,
              verified: false,
              errorMessage: connect.message,
            },
          });
          logWizardEvent({
            category: "authentication",
            severity: "medium",
            activityName: "verify_auth_fail",
            message: connect.message,
            component: "AuthVerificationStep",
            attributes: { targetId },
          });
          return { ok: false, message: connect.message };
        }

        step1PassedRef.current = { key: probeKey };
        logWizardEvent({
          category: "authentication",
          activityName: "verify_auth_ok",
          message: "Connectivity / authentication probe succeeded",
          component: "AuthVerificationStep",
          attributes: { targetId },
        });
      }

      const yazg = await assertYazgAgentLive(true);
      if (!yazg.live) {
        setVerifyPhase("failed_ai");
        activePhase = "failed_ai";
        setVerifyResultMessage(yazg.message);
        onError(yazg.message);
        append(formatErrorLogLine(yazg.message));
        return { ok: false, message: yazg.message };
      }

      activePhase = "ai";
      setVerifyPhase(activePhase);
      append(VERIFICATION_LOG_START_AI);
      append(formatSendRequestLogLine(capabilityPreview.requestLog, 2));

      const result = await verifyTargetProfileAi(
        targetId,
        profileToPayload(profileRef.current),
        verifyOptions,
      );
      if (result.probeConsole) {
        append(formatResponseLogLine(result.probeConsole, "ai_probe"));
      }
      append(formatAiValidationLogLine(result.message));

      const nextProfile = profileFromDto(result.profile);
      if (result.verified) {
        nextProfile.verification = {
          ...nextProfile.verification,
          verified: true,
          status: nextProfile.verification.status || "verified",
          errorMessage: null,
        };
      }
      onProfileChange(nextProfile);
      onVerifySettled?.();
      setVerifyResultMessage(result.message);
      if (result.verified) {
        setVerifyPhase("done");
        activePhase = "done";
        onError(null);
        if (!quietToasts) notify(result.message, "success");
        onVerifySuccess?.();
        logWizardEvent({
          category: "authentication",
          activityName: "verify_ai_ok",
          message: result.message || "Endpoint verified as an AI system",
          component: "AuthVerificationStep",
          attributes: { targetId },
        });
        return { ok: true, message: result.message };
      }

      setVerifyPhase("failed_ai");
      activePhase = "failed_ai";
      onError(result.message);
      if (!quietToasts) notify(result.message, "error");
      logWizardEvent({
        category: "authentication",
        severity: "medium",
        activityName: "verify_ai_fail",
        message: result.message,
        component: "AuthVerificationStep",
        attributes: { targetId },
      });
      return { ok: false, message: result.message };
    } catch (err) {
      const message = err instanceof Error ? err.message : "Verification failed";
      setVerifyResultMessage(message);
      setVerifyPhase(activePhase === "auth" ? "failed_auth" : "failed_ai");
      onError(message);
      if (!quietToasts) notify(message, "error");
      append(formatErrorLogLine(message));
      onProfileChange({
        verification: {
          ...profileRef.current.verification,
          verified: false,
          errorMessage: message,
        },
      });
      return { ok: false, message };
    } finally {
      setVerifying(false);
    }
  }

  async function handleVerify() {
    await runVerifyOnce({ quietToasts: false });
  }

  useEffect(() => {
    if (!autoVerify || profile.verification.verified) {
      return;
    }

    let cancelled = false;

    void (async () => {
      // Let auth inference from headers settle before first probe.
      await sleep(400);
      if (cancelled) return;

      let lastMessage: string | null = null;
      for (let attempt = 1; attempt <= autoVerifyMaxAttempts; attempt += 1) {
        if (cancelled) return;
        onAutoVerifyAttempt?.(attempt, autoVerifyMaxAttempts);
        const result = await runVerifyOnce({ quietToasts: true });
        lastMessage = result.message;
        if (result.ok) {
          if (!cancelled) {
            onAutoVerifyComplete?.({ ok: true, attempts: attempt, message: result.message });
          }
          return;
        }
        if (attempt < autoVerifyMaxAttempts) {
          if (!cancelled) {
            const next = appendVerificationLogLine(
              verificationLogRef.current,
              `Import harness: verification failed (attempt ${attempt}/${autoVerifyMaxAttempts}) — retrying…`,
            );
            verificationLogRef.current = next;
            onVerificationLog(next);
            logWizardEvent({
              category: "harness",
              severity: "low",
              activityName: "import_verify_retry",
              message: `Import verification retry ${attempt + 1}/${autoVerifyMaxAttempts}`,
              component: "AuthVerificationStep",
              attributes: { targetId, attempt, maxAttempts: autoVerifyMaxAttempts },
            });
          }
          await sleep(IMPORT_VERIFY_RETRY_DELAY_MS);
        }
      }
      if (!cancelled) {
        onAutoVerifyComplete?.({
          ok: false,
          attempts: autoVerifyMaxAttempts,
          message: lastMessage,
        });
      }
    })();

    return () => {
      cancelled = true;
    };
    // Intentionally start once per autoVerify/target gate.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- harness start gate
  }, [autoVerify, targetId]);

  // Failure detail is shown once in the pipeline (uses verifyResultMessage).
  // Avoid duplicating the same text under the Verification button.
  const verificationButtonError =
    verifyPhase === "failed_auth" || verifyPhase === "failed_ai"
      ? null
      : error;

  return (
    <div className="auth-verification-step">
      <section>
        {authDetectedFromProfile && authForm.authKind !== "none" && (
          <p className="text-muted text-sm auth-verification-step__detected">
            Detected <strong>{authMethodLabel(authForm.authKind)}</strong> from Step 2 headers.
            Adjust below if needed.
          </p>
        )}
        <TargetFormFields
          form={authForm}
          authKindLabel="Authentication Type"
          onChange={(patch) => {
            if (patch.authKind !== undefined) {
              authKindChosenByUserRef.current = true;
              if (patch.authKind !== authForm.authKind) {
                setAuthDetectedFromProfile(false);
              }
            }
            onAuthChange(patch);
          }}
          hideUrl
        />
      </section>

      <section>
        <div className="auth-verification-step__section-head">
          <p className="text-muted text-sm">
            Sends a capability-inventory probe to your endpoint, then uses Yazg to confirm the
            response is from a generative AI system and capture signals for attack planning.
          </p>
        </div>
        <Button
          variant="primary"
          disabled={verifying || autoVerify}
          onClick={() => void handleVerify()}
        >
          <span className="btn__content">
            <IconAi className="btn__icon" aria-hidden />
            {verifying ? "Verifying…" : autoVerify ? "Auto-verifying…" : "Verification"}
          </span>
        </Button>
        {verificationButtonError ? (
          <p className="text-danger text-sm auth-verification-step__error" role="alert">
            {verificationButtonError}
          </p>
        ) : null}
        <VerificationProgressPipeline phase={verifyPhase} resultMessage={verifyResultMessage} />
      </section>

      <section>
        <VerificationConsole lines={verificationLog} pending={verifying} />
      </section>
    </div>
  );
}
