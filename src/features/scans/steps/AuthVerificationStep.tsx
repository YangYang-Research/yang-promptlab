import { useState } from "react";

import { Badge, Button } from "@/shared/components";
import { verifyTargetProfile } from "@/shared/ipc/targetProfile";
import { useToast } from "@/shared/notifications";

import { TargetFormFields } from "../TargetFormFields";
import { VerificationConsole } from "../components/VerificationConsole";
import {
  profileFromDto,
  profileToPayload,
  type TargetProfileFormState,
  type VerificationConsoleEntryDto,
} from "../targetProfile";
import type { TargetFormState } from "../targetDescriptor";

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

  async function handleVerify() {
    setVerifying(true);
    onError(null);
    try {
      if (onBeforeVerify) {
        const ready = await onBeforeVerify();
        if (!ready) return;
      }
      const result = await verifyTargetProfile(targetId, profileToPayload(profile));
      onProfileChange(profileFromDto(result.profile));
      onVerificationConsole(result.console);
      notify(result.message, "success");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Verification failed";
      onError(message);
      notify(message, "error");
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
        <TargetFormFields form={authForm} onChange={onAuthChange} />
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
        <VerificationConsole entry={verificationConsole} />
      </section>
    </div>
  );
}
