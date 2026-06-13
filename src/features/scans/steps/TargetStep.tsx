import { Button } from "@/shared/components";
import type { TargetFormState, TargetAuthKind } from "../targetDescriptor";

const AUTH_OPTIONS: { value: TargetAuthKind; label: string }[] = [
  { value: "none", label: "None" },
  { value: "basic", label: "Username / password" },
  { value: "sso", label: "SSO" },
  { value: "api_key", label: "API key" },
];

type TargetStepProps = {
  form: TargetFormState;
  onChange: (patch: Partial<TargetFormState>) => void;
  error: string | null;
};

export function TargetStep({ form, onChange, error }: TargetStepProps) {
  return (
    <div className="wizard-step">
      <div className="project-form wizard-target-form">
        <label className="field">
          <span className="field__label">Target URL</span>
          <input
            className="input"
            type="url"
            placeholder="https://api.example.com/v1/chat"
            value={form.url}
            onChange={(e) => onChange({ url: e.target.value })}
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
                  checked={form.authKind === option.value}
                  onChange={() =>
                    onChange({
                      authKind: option.value,
                      ssoSessionReady: option.value === "sso" ? form.ssoSessionReady : false,
                    })
                  }
                />
                <span>{option.label}</span>
              </label>
            ))}
          </div>
        </fieldset>

        {form.authKind === "basic" && (
          <div className="wizard-auth-fields">
            <label className="field">
              <span className="field__label">Username</span>
              <input
                className="input"
                value={form.username}
                onChange={(e) => onChange({ username: e.target.value })}
                autoComplete="username"
              />
            </label>
            <label className="field">
              <span className="field__label">Password</span>
              <input
                className="input"
                type="password"
                value={form.password}
                onChange={(e) => onChange({ password: e.target.value })}
                autoComplete="current-password"
              />
            </label>
          </div>
        )}

        {form.authKind === "sso" && (
          <div className="wizard-auth-fields wizard-auth-fields--sso">
            <p className="text-muted text-sm">
              Authenticate in a browser session. Playwright integration will capture cookies and
              tokens automatically.
            </p>
            <Button
              type="button"
              variant="secondary"
              onClick={() => onChange({ ssoSessionReady: true })}
            >
              Launch Browser Authentication
            </Button>
            {form.ssoSessionReady && (
              <p className="text-muted text-sm">
                Browser authentication placeholder — session will be wired to Playwright in a
                future release.
              </p>
            )}
          </div>
        )}

        {form.authKind === "api_key" && (
          <div className="wizard-auth-fields">
            <label className="field">
              <span className="field__label">Header name</span>
              <input
                className="input"
                placeholder="Authorization"
                value={form.headerName}
                onChange={(e) => onChange({ headerName: e.target.value })}
              />
            </label>
            <label className="field">
              <span className="field__label">API key</span>
              <input
                className="input"
                type="password"
                placeholder="sk-…"
                value={form.headerValue}
                onChange={(e) => onChange({ headerValue: e.target.value })}
                autoComplete="off"
              />
            </label>
          </div>
        )}

        {error && <p className="text-danger">{error}</p>}
      </div>
    </div>
  );
}
