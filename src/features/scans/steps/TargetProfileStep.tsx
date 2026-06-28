import { useEffect, useMemo, useState } from "react";

import { Badge, Select } from "@/shared/components";
import { listTargetProfileTemplates } from "@/shared/ipc/targetProfile";

import {
  PROMPT_PLACEHOLDER,
  PROVIDER_OPTIONS,
  applyApiEndpointToProfile,
  createEmptyVerification,
  formatApiEndpoint,
  profileFromDto,
  type TargetProfileFormState,
} from "../targetProfile";

type TargetProfileStepProps = {
  profile: TargetProfileFormState;
  onChange: (patch: Partial<TargetProfileFormState>) => void;
  error: string | null;
};

export function TargetProfileStep({ profile, onChange, error }: TargetProfileStepProps) {
  const [loadingTemplates, setLoadingTemplates] = useState(false);
  const apiEndpoint = useMemo(() => formatApiEndpoint(profile), [profile.baseUrl, profile.path]);

  useEffect(() => {
    let cancelled = false;
    setLoadingTemplates(true);
    void listTargetProfileTemplates()
      .then((templates) => {
        if (cancelled || templates.length === 0) return;
        const match = templates.find((t) => t.provider === profile.provider);
        if (match && !profile.requestTemplate.trim()) {
          onChange(profileFromDto(match));
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingTemplates(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const hasPlaceholder = useMemo(
    () => profile.requestTemplate.includes(profile.promptPlaceholder || PROMPT_PLACEHOLDER),
    [profile.requestTemplate, profile.promptPlaceholder],
  );

  function handleEndpointChange(value: string) {
    onChange(applyApiEndpointToProfile(value));
  }

  function applyProvider(provider: TargetProfileFormState["provider"]) {
    void listTargetProfileTemplates().then((templates) => {
      const template = templates.find((t) => t.provider === provider);
      if (template) {
        onChange({
          ...profileFromDto(template),
          verification: createEmptyVerification(),
        });
      } else {
        onChange({ provider, verification: createEmptyVerification() });
      }
    });
  }

  return (
    <div className="project-form wizard-target-form">
      <div className="wizard-target-form__row">
        <label className="field wizard-target-form__type">
          <span className="field__label">Type</span>
          <Select
            value={profile.provider}
            disabled={loadingTemplates}
            onChange={(e) => applyProvider(e.target.value as TargetProfileFormState["provider"])}
          >
            {PROVIDER_OPTIONS.map((option) => (
              <option key={option.id} value={option.id}>
                {option.label}
              </option>
            ))}
          </Select>
        </label>

        <label className="field wizard-target-form__method">
          <span className="field__label">Method</span>
          <Select
            value={profile.method === "GET" ? "GET" : "POST"}
            onChange={(e) => onChange({ method: e.target.value })}
          >
            <option value="POST">POST</option>
            <option value="GET">GET</option>
          </Select>
        </label>

        <label className="field wizard-target-form__endpoint">
          <span className="field__label">AI API Endpoint</span>
          <input
            className="input wizard-target-form__mono"
            type="url"
            value={apiEndpoint}
            onChange={(e) => handleEndpointChange(e.target.value)}
            placeholder="https://api.openai.com/v1/chat/completions"
            autoComplete="url"
            autoFocus
          />
        </label>
      </div>
      <span className="text-muted text-sm wizard-target-form__hint">
        Full URL including path and query string if needed.
      </span>

      <label className="field">
        <span className="field__label">Headers (JSON)</span>
        <textarea
          className="input textarea wizard-target-form__mono"
          rows={4}
          value={profile.headersJson}
          onChange={(e) => onChange({ headersJson: e.target.value })}
          spellCheck={false}
        />
      </label>

      <div className="field">
        <div className="wizard-target-form__label-row">
          <span className="field__label">Body template</span>
          <Badge variant={hasPlaceholder ? "success" : "warning"}>
            {hasPlaceholder ? `${PROMPT_PLACEHOLDER} present` : `Missing ${PROMPT_PLACEHOLDER}`}
          </Badge>
        </div>
        <textarea
          className="input textarea wizard-target-form__mono wizard-target-form__template"
          rows={12}
          value={profile.requestTemplate}
          onChange={(e) => onChange({ requestTemplate: e.target.value })}
          spellCheck={false}
        />
        <span className="text-muted text-sm">
          Payload Generator replaces only <code>{PROMPT_PLACEHOLDER}</code> — other JSON fields are
          never modified.
        </span>
      </div>

      {error && <p className="text-danger">{error}</p>}
    </div>
  );
}
