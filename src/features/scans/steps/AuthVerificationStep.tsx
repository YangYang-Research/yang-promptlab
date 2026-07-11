import { useEffect, useRef, useState } from "react";

import { Button, YazgBadge } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import {
  verifyTargetProfileAi,
  verifyTargetProfileConnect,
} from "@/shared/ipc/targetProfile";
import { useToast } from "@/shared/notifications";

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
import { buildVerificationRequestPreview, authHeadersFromForm, CONNECT_PROBE_PROMPT, VERIFY_PROMPT } from "../verificationRequest";
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

type AuthVerificationStepProps = {
  targetId: string;
  profile: TargetProfileFormState;
  onProfileChange: (patch: Partial<TargetProfileFormState>) => void;
  authForm: TargetFormState;
  onAuthChange: (patch: Partial<TargetFormState>) => void;
  verificationLog: VerificationLogLine[];
  onVerificationLog: (lines: VerificationLogLine[]) => void;
  onError: (message: string | null) => void;
  onBeforeVerify?: () => Promise<boolean>;
  onVerifySuccess?: () => void;
  onVerifySettled?: () => void;
};

function authMethodLabel(kind: TargetFormState["authKind"]): string {
  return AUTH_METHOD_OPTIONS.find((option) => option.value === kind)?.label ?? kind;
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
  onBeforeVerify,
  onVerifySuccess,
  onVerifySettled,
}: AuthVerificationStepProps) {
  const { notify } = useToast();
  const [verifying, setVerifying] = useState(false);
  const [verifyPhase, setVerifyPhase] = useState<VerificationPipelinePhase>("idle");
  const [verifyResultMessage, setVerifyResultMessage] = useState<string | null>(null);
  const [authDetectedFromProfile, setAuthDetectedFromProfile] = useState(false);
  const authFormRef = useRef(authForm);
  authFormRef.current = authForm;
  const onAuthChangeRef = useRef(onAuthChange);
  onAuthChangeRef.current = onAuthChange;
  /** Step 1 passed for this probe key — retry after Step 2 failure skips Step 1. */
  const step1PassedRef = useRef<{ key: string } | null>(null);
  const verifyPhaseRef = useRef(verifyPhase);
  verifyPhaseRef.current = verifyPhase;

  useEffect(() => {
    const current = authFormRef.current;
    const inferred = inferAuthFromProfileHeaders(profile, current);

    if (current.authKind === "none") {
      if (inferred.authKind && inferred.authKind !== "none") {
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

  async function handleVerify() {
    const connectPreview = buildVerificationRequestPreview(profile, authFormRef.current, {
      prompt: CONNECT_PROBE_PROMPT,
    });
    const capabilityPreview = buildVerificationRequestPreview(profile, authFormRef.current, {
      prompt: VERIFY_PROMPT,
    });
    const probeKey = connectProbeKey(profile, authFormRef.current);
    const skipStep1 =
      verifyPhaseRef.current === "failed_ai" &&
      step1PassedRef.current?.key === probeKey;

    setVerifying(true);
    let activePhase: VerificationPipelinePhase = skipStep1 ? "ai" : "auth";
    setVerifyPhase(activePhase);
    setVerifyResultMessage(null);
    onError(null);

    let log: VerificationLogLine[] = skipStep1 ? [...verificationLog] : [];
    const publishLog = () => onVerificationLog([...log]);
    const append = (message: string) => {
      log = appendVerificationLogLine(log, message);
      publishLog();
    };
    const appendMany = (messages: string[]) => {
      log = appendVerificationLogLines(log, messages);
      publishLog();
    };

    const profileUrl = fullProfileUrl(profile);
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
          onError("Authentication was not saved — fix errors above and retry.");
          append(formatErrorLogLine("Authentication was not saved — fix errors above and retry."));
          return;
        }
      }

      if (!skipStep1) {
        const connect = await verifyTargetProfileConnect(
          targetId,
          profileToPayload(profile),
          verifyOptions,
        );
        append(formatResponseLogLine(connect.console, "connectivity"));

        if (!connect.success) {
          step1PassedRef.current = null;
          setVerifyPhase("failed_auth");
          activePhase = "failed_auth";
          onError(connect.message);
          notify(connect.message, "error");
          onProfileChange({
            verification: {
              ...profile.verification,
              verified: false,
              errorMessage: connect.message,
            },
          });
          return;
        }

        step1PassedRef.current = { key: probeKey };
      }

      activePhase = "ai";
      setVerifyPhase(activePhase);
      append(VERIFICATION_LOG_START_AI);
      append(formatSendRequestLogLine(capabilityPreview.requestLog, 2));

      // Step 2 re-sends the capability probe, then Yazg analyzes that fresh response.
      const result = await verifyTargetProfileAi(
        targetId,
        profileToPayload(profile),
        verifyOptions,
      );
      if (result.probeConsole) {
        append(formatResponseLogLine(result.probeConsole, "ai_probe"));
      }
      append(formatAiValidationLogLine(result.message));

      onProfileChange(profileFromDto(result.profile));
      onVerifySettled?.();
      setVerifyResultMessage(result.message);
      if (result.verified) {
        setVerifyPhase("done");
        activePhase = "done";
        onError(null);
        notify(result.message, "success");
        onVerifySuccess?.();
      } else {
        setVerifyPhase("failed_ai");
        activePhase = "failed_ai";
        onError(result.message);
        notify(result.message, "error");
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Verification failed";
      setVerifyResultMessage(message);
      setVerifyPhase(activePhase === "auth" ? "failed_auth" : "failed_ai");
      onError(message);
      notify(message, "error");
      append(formatErrorLogLine(message));
      onProfileChange({
        verification: {
          ...profile.verification,
          verified: false,
          errorMessage: message,
        },
      });
    } finally {
      setVerifying(false);
    }
  }

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
            if (patch.authKind && patch.authKind !== authForm.authKind) {
              setAuthDetectedFromProfile(false);
            }
            onAuthChange(patch);
          }}
          hideUrl
        />
      </section>

      <section>
        <div className="auth-verification-step__section-head">
          <p className="text-muted text-sm">
            Sends a capability-inventory probe to your endpoint, then uses Yazg to
            confirm the response is from a generative AI system and capture signals for attack planning.
          </p>
          {profile.verification.verified ? <YazgBadge /> : null}
        </div>
        <Button variant="primary" disabled={verifying} onClick={() => void handleVerify()}>
          <span className="btn__content">
            <IconAi className="btn__icon" aria-hidden />
            {verifying ? "Verifying…" : "Verification"}
          </span>
        </Button>
        <VerificationProgressPipeline phase={verifyPhase} resultMessage={verifyResultMessage} />
      </section>

      <section>
        <VerificationConsole lines={verificationLog} pending={verifying} />
      </section>
    </div>
  );
}
