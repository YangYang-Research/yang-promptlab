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
import { buildVerificationRequestPreview, authHeadersFromForm } from "../verificationRequest";
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

  async function handleVerify() {
    const preview = buildVerificationRequestPreview(profile, authFormRef.current);
    setVerifying(true);
    let activePhase: VerificationPipelinePhase = "auth";
    setVerifyPhase(activePhase);
    setVerifyResultMessage(null);
    onError(null);

    let log: VerificationLogLine[] = [];
    const publishLog = () => onVerificationLog([...log]);
    const append = (message: string) => {
      log = appendVerificationLogLine(log, message);
      publishLog();
    };
    const appendMany = (messages: string[]) => {
      log = appendVerificationLogLines(log, messages);
      publishLog();
    };

    appendMany([
      VERIFICATION_LOG_START_AUTH,
      ...formatAuthenticationLogLines(preview.headers),
      formatSendRequestLogLine(preview.requestLog),
    ]);

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

      const profileUrl = fullProfileUrl(profile);
      const descriptor = buildTargetDescriptor({
        ...authFormRef.current,
        url: profileUrl,
      }) as { auth?: Record<string, unknown> };
      const verifyOptions = {
        auth: descriptor.auth ?? null,
        authHeaders: authHeadersFromForm(authFormRef.current),
      };

      const connect = await verifyTargetProfileConnect(
        targetId,
        profileToPayload(profile),
        verifyOptions,
      );
      append(formatResponseLogLine(connect.console, "connectivity"));

      if (!connect.success || !connect.connectSnapshot) {
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

      activePhase = "ai";
      setVerifyPhase(activePhase);
      append(VERIFICATION_LOG_START_AI);

      const result = await verifyTargetProfileAi(
        targetId,
        profileToPayload(profile),
        connect.connectSnapshot,
      );
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
