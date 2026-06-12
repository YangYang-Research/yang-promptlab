import { useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Button } from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { Target } from "@/shared/types";

import {
  buildTargetDescriptor,
  deriveTargetName,
  validateTargetStep,
  type TargetAuthKind,
} from "../targetDescriptor";

const AUTH_OPTIONS: { value: TargetAuthKind; label: string }[] = [
  { value: "none", label: "None" },
  { value: "basic", label: "Username / password" },
  { value: "api_key", label: "API key" },
];

type TargetStepProps = {
  projectId: string;
  onTargetSaved?: (target: Target) => void;
};

export function TargetStep({ projectId, onTargetSaved }: TargetStepProps) {
  const { actions } = useAppStore();
  const { notify } = useToast();

  const [url, setUrl] = useState("");
  const [authKind, setAuthKind] = useState<TargetAuthKind>("none");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [headerName, setHeaderName] = useState("Authorization");
  const [headerValue, setHeaderValue] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [savedTarget, setSavedTarget] = useState<Target | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (savedTarget) return;

    const input = {
      url,
      authKind,
      username,
      password,
      headerName,
      headerValue,
    };
    const validationError = validateTargetStep(input);
    if (validationError) {
      setFormError(validationError);
      return;
    }

    setSubmitting(true);
    setFormError(null);

    try {
      const descriptor = buildTargetDescriptor(input);
      const name = deriveTargetName(url);
      const target = await actions.createTarget(projectId, name, "web", descriptor);
      setSavedTarget(target);
      onTargetSaved?.(target);
      notify(`Target "${name}" saved`, "success");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to save target";
      setFormError(message);
      notify(message, "error");
      setSubmitting(false);
    }
  }

  return (
    <div className="wizard-step">
      <div className="wizard-step__heading">
        <span className="wizard-step__number">2</span>
        <div>
          <h3 className="wizard-step__title">Target &amp; authentication</h3>
          <p className="wizard-step__hint text-muted">
            Enter the scan target URL and optional credentials
          </p>
        </div>
      </div>

      {savedTarget ? (
        <div className="wizard-target-saved">
          <Badge variant="success">Saved to SQLite</Badge>
          <p>
            <strong>{savedTarget.name}</strong>
          </p>
          <p className="text-muted text-sm">{savedTarget.url}</p>
          <p className="text-muted text-sm">Target ID: {savedTarget.id}</p>
        </div>
      ) : (
        <form className="project-form wizard-target-form" onSubmit={handleSubmit}>
          <label className="field">
            <span className="field__label">Target URL</span>
            <input
              className="input"
              type="url"
              placeholder="https://api.example.com/v1/chat"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              autoComplete="url"
            />
          </label>

          <fieldset className="wizard-auth-fieldset">
            <legend className="field__label">Authentication</legend>
            <div className="wizard-auth-options">
              {AUTH_OPTIONS.map((option) => (
                <label key={option.value} className="wizard-auth-option">
                  <input
                    type="radio"
                    name="authKind"
                    value={option.value}
                    checked={authKind === option.value}
                    onChange={() => setAuthKind(option.value)}
                  />
                  <span>{option.label}</span>
                </label>
              ))}
            </div>
          </fieldset>

          {authKind === "basic" && (
            <div className="wizard-auth-fields">
              <label className="field">
                <span className="field__label">Username</span>
                <input
                  className="input"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  autoComplete="username"
                />
              </label>
              <label className="field">
                <span className="field__label">Password</span>
                <input
                  className="input"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="current-password"
                />
              </label>
            </div>
          )}

          {authKind === "api_key" && (
            <div className="wizard-auth-fields">
              <label className="field">
                <span className="field__label">Header name</span>
                <input
                  className="input"
                  placeholder="Authorization"
                  value={headerName}
                  onChange={(e) => setHeaderName(e.target.value)}
                />
              </label>
              <label className="field">
                <span className="field__label">API key</span>
                <input
                  className="input"
                  type="password"
                  placeholder="sk-…"
                  value={headerValue}
                  onChange={(e) => setHeaderValue(e.target.value)}
                  autoComplete="off"
                />
              </label>
            </div>
          )}

          {formError && <p className="text-danger">{formError}</p>}

          <div className="project-form__actions">
            <Button variant="primary" type="submit" disabled={submitting || !url.trim()}>
              {submitting ? "Saving…" : "Save Target"}
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
