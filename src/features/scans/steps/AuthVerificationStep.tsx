import { useEffect, useRef, useState } from "react";

import { Badge, Button } from "@/shared/components";
import { verifyTargetProfile } from "@/shared/ipc/targetProfile";
import { useToast } from "@/shared/notifications";

import { TargetFormFields } from "../TargetFormFields";
import { VerificationConsole } from "../components/VerificationConsole";
import {
  AUTH_METHOD_OPTIONS,
  buildTargetDescriptor,
  inferAuthFromProfileHeaders,
  type TargetFormState,
} from "../targetDescriptor";
import {
  fullProfileUrl,
  profileFromDto,
  profileToPayload,
  type TargetProfileFormState,
  type VerificationConsoleEntryDto,
} from "../targetProfile";
import { buildVerificationRequestPreview, authHeadersFromForm } from "../verificationRequest";

type AuthVerificationStepProps = {
  targetId: string;
  profile: TargetProfileFormState;
  onProfileChange: (patch: Partial<TargetProfileFormState>) => void;
  authForm: TargetFormState;
  onAuthChange: (patch: Partial<TargetFormState>) => void;
  verificationConsole: VerificationConsoleEntryDto | null;
  onVerificationConsole: (entry: VerificationConsoleEntryDto | null) => void;
  error: string | null;
  onError: (message: string | null) => void;
  onBeforeVerify?: () => Promise<boolean>;
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
  verificationConsole,
  onVerificationConsole,
  error,
  onError,
  onBeforeVerify,
}: AuthVerificationStepProps) {
  const { notify } = useToast();
  const [verifying, setVerifying] = useState(false);
  const [authDetectedFromProfile, setAuthDetectedFromProfile] = useState(false);
  const authFormRef = useRef(authForm);
  authFormRef.current = authForm;
  const onAuthChangeRef = useRef(onAuthChange);
  onAuthChangeRef.current = onAuthChange;

  useEffect(() => {
    const inferred = inferAuthFromProfileHeaders(profile, authFormRef.current);
    if (inferred.authKind && inferred.authKind !== "none") {
      setAuthDetectedFromProfile(true);
    }
    onAuthChangeRef.current(inferred);
  }, [profile.headersJson, profile.baseUrl, profile.path, targetId]);

  function mergeConsoleWithPreview(
    backend: VerificationConsoleEntryDto,
    preview: ReturnType<typeof buildVerificationRequestPreview>,
  ): VerificationConsoleEntryDto {
    return {
      ...backend,
      requestLog: preview.requestLog,
      authDebug: preview.authDebug,
    };
  }

  async function handleVerify() {
    const preview = buildVerificationRequestPreview(profile, authFormRef.current);
    setVerifying(true);
    onError(null);
    onVerificationConsole(preview);

    try {
      if (onBeforeVerify) {
        const ready = await onBeforeVerify();
        if (!ready) {
          onVerificationConsole({
            ...preview,
            message: "Authentication was not saved — fix errors above and retry.",
          });
          return;
        }
      }

      const profileUrl = fullProfileUrl(profile);
      const descriptor = buildTargetDescriptor({
        ...authFormRef.current,
        url: profileUrl,
      }) as { auth?: Record<string, unknown> };
      const result = await verifyTargetProfile(targetId, profileToPayload(profile), {
        auth: descriptor.auth ?? null,
        authHeaders: authHeadersFromForm(authFormRef.current),
      });
      onVerificationConsole(mergeConsoleWithPreview(result.console, preview));
      onProfileChange(profileFromDto(result.profile));
      if (result.verified) {
        onError(null);
        notify(result.message, "success");
      } else {
        onError(result.message);
        notify(result.message, "error");
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Verification failed";
      onError(message);
      notify(message, "error");
      onVerificationConsole({
        ...preview,
        message,
      });
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
      <div className="auth-verification-step__status">
        <Badge variant={profile.verification.verified ? "success" : "warning"}>
          {profile.verification.verified ? "Verified" : "Not verified"}
        </Badge>
        {profile.verification.model && (
          <span className="text-muted">Model: {profile.verification.model}</span>
        )}
        {profile.verification.verified && profile.verification.responseTimeMs > 0 && (
          <span className="text-muted">{profile.verification.responseTimeMs}ms</span>
        )}
      </div>

      <section>
        <h3 className="wizard-section-title">Authentication</h3>
        {authDetectedFromProfile && authForm.authKind !== "none" && (
          <p className="text-muted text-sm auth-verification-step__detected">
            Detected <strong>{authMethodLabel(authForm.authKind)}</strong> from Step 2 headers.
            Adjust below if needed.
          </p>
        )}
        <TargetFormFields
          form={authForm}
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
        <h3 className="wizard-section-title">Verify connection</h3>
        <p className="text-muted">
          Sends a real AI request (<code>Hello</code>) using your Target Profile. Verification does
          not use AI Runtime.
        </p>
        <Button variant="primary" disabled={verifying} onClick={() => void handleVerify()}>
          {verifying ? "Verifying…" : "Verify Connection"}
        </Button>
        {error && <p className="text-danger">{error}</p>}
      </section>

      <section>
        <h3 className="wizard-section-title">Verification console</h3>
        <VerificationConsole entry={verificationConsole} pending={verifying} />
      </section>
    </div>
  );
}
