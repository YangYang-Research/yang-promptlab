import {
  AUTH_METHOD_OPTIONS,
  type TargetAuthKind,
  type TargetFormState,
} from "./targetDescriptor";
import { PlaywrightRecordPanel } from "./PlaywrightRecordPanel";

type TargetFormFieldsProps = {
  form: TargetFormState;
  onChange: (patch: Partial<TargetFormState>) => void;
  error?: string | null;
  autoFocusUrl?: boolean;
  /** Wizard Step 3 — endpoint comes from AI Target Profile (Step 2). */
  hideUrl?: boolean;
  /** Label for the auth method selector (default: Authentication). */
  authKindLabel?: string;
};

const AUTH_BUTTON_CLASS: Record<TargetAuthKind, string> = {
  none: "wizard-auth-btn--none",
  username_password: "wizard-auth-btn--username",
  sso: "wizard-auth-btn--sso",
  basic: "wizard-auth-btn--basic",
  api_key: "wizard-auth-btn--api-key",
  jwt: "wizard-auth-btn--jwt",
};

function selectAuthKind(onChange: TargetFormFieldsProps["onChange"], kind: TargetAuthKind) {
  onChange({
    authKind: kind,
    browserSessionReady: false,
    browserSessionId: null,
  });
}

export function TargetFormFields({
  form,
  onChange,
  error,
  autoFocusUrl = false,
  hideUrl = false,
  authKindLabel = "Authentication",
}: TargetFormFieldsProps) {
  return (
    <div className="project-form wizard-target-form">
      {!hideUrl && (
        <label className="field">
          <span className="field__label">Target URL</span>
          <input
            className="input"
            type="url"
            placeholder="https://api.example.com/v1/chat"
            value={form.url}
            onChange={(e) =>
              onChange({
                url: e.target.value,
                browserSessionReady: false,
                browserSessionId: null,
              })
            }
            autoComplete="url"
            autoFocus={autoFocusUrl}
          />
        </label>
      )}

      <div className="wizard-auth-fieldset" role="radiogroup" aria-label={authKindLabel}>
        <span className="field__label">{authKindLabel}</span>
        <div className="wizard-auth-buttons">
          {AUTH_METHOD_OPTIONS.map((option) => {
            const isActive = form.authKind === option.value;
            return (
              <button
                key={option.value}
                type="button"
                role="radio"
                aria-checked={isActive}
                className={[
                  "wizard-auth-btn",
                  isActive ? AUTH_BUTTON_CLASS[option.value] : "",
                  isActive ? "wizard-auth-btn--selected" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => selectAuthKind(onChange, option.value)}
              >
                {option.label}
              </button>
            );
          })}
        </div>
      </div>

      {form.authKind !== "none" && (
        <div className="wizard-auth-panel">
          {form.authKind === "username_password" && (
            <div className="wizard-auth-fields">
              <label className="field">
                <span className="field__label">Username</span>
                <input
                  className="input"
                  value={form.loginUsername}
                  onChange={(e) =>
                    onChange({
                      loginUsername: e.target.value,
                      browserSessionReady: false,
                      browserSessionId: null,
                    })
                  }
                  autoComplete="username"
                />
              </label>
              <label className="field">
                <span className="field__label">Password</span>
                <input
                  className="input"
                  type="password"
                  value={form.loginPassword}
                  onChange={(e) =>
                    onChange({
                      loginPassword: e.target.value,
                      browserSessionReady: false,
                      browserSessionId: null,
                    })
                  }
                  autoComplete="current-password"
                />
              </label>
              <PlaywrightRecordPanel
                form={form}
                authKind="username_password"
                onChange={onChange}
                startLabel="Record login session"
              />
            </div>
          )}

          {form.authKind === "sso" && (
            <PlaywrightRecordPanel
              form={form}
              authKind="sso"
              onChange={onChange}
              startLabel="Launch browser authentication"
            />
          )}

          {form.authKind === "basic" && (
            <div className="wizard-auth-fields">
              <label className="field">
                <span className="field__label">Username</span>
                <input
                  className="input"
                  value={form.basicUsername}
                  onChange={(e) => onChange({ basicUsername: e.target.value })}
                  autoComplete="username"
                />
              </label>
              <label className="field">
                <span className="field__label">Password</span>
                <input
                  className="input"
                  type="password"
                  value={form.basicPassword}
                  onChange={(e) => onChange({ basicPassword: e.target.value })}
                  autoComplete="current-password"
                />
              </label>
            </div>
          )}

          {form.authKind === "api_key" && (
            <div className="wizard-auth-fields">
              <label className="field">
                <span className="field__label">Header name</span>
                <input
                  className="input"
                  placeholder="Authorization"
                  value={form.apiKeyHeaderName}
                  onChange={(e) => onChange({ apiKeyHeaderName: e.target.value })}
                />
              </label>
              <label className="field">
                <span className="field__label">Prefix (optional)</span>
                <input
                  className="input"
                  placeholder="Bearer "
                  value={form.apiKeyPrefix}
                  onChange={(e) => onChange({ apiKeyPrefix: e.target.value })}
                />
              </label>
              <label className="field">
                <span className="field__label">API key</span>
                <input
                  className="input"
                  type="password"
                  placeholder={form.apiKeyVaultMissing ? "Re-enter API key (missing from keychain)" : "sk-…"}
                  value={form.apiKeyValue}
                  onChange={(e) =>
                    onChange({ apiKeyValue: e.target.value, apiKeyVaultMissing: false })
                  }
                  autoComplete="off"
                />
              </label>
              {form.apiKeyVaultMissing && (
                <p className="text-warning text-sm">
                  Stored API key is no longer in the system keychain. Enter it again to continue.
                </p>
              )}
            </div>
          )}

          {form.authKind === "jwt" && (
            <div className="wizard-auth-fields">
              <label className="field">
                <span className="field__label">Header name</span>
                <input
                  className="input"
                  placeholder="Authorization"
                  value={form.jwtHeaderName}
                  onChange={(e) => onChange({ jwtHeaderName: e.target.value })}
                />
              </label>
              <label className="field">
                <span className="field__label">Prefix (optional)</span>
                <input
                  className="input"
                  placeholder="Bearer "
                  value={form.jwtPrefix}
                  onChange={(e) => onChange({ jwtPrefix: e.target.value })}
                />
              </label>
              <label className="field">
                <span className="field__label">JWT token</span>
                <input
                  className="input"
                  type="password"
                  placeholder="eyJhbG…"
                  value={form.jwtToken}
                  onChange={(e) => onChange({ jwtToken: e.target.value })}
                  autoComplete="off"
                />
              </label>
            </div>
          )}
        </div>
      )}

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
