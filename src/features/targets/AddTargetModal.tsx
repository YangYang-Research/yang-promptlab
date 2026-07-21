import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { curlToProfilePatch, targetFormAuthFromHeaders } from "@/features/scans/curlImport";
import { TargetFormFields } from "@/features/scans/TargetFormFields";
import {
  buildTargetDescriptor,
  createInitialTargetForm,
  deriveTargetName,
  validateTargetStep,
  type TargetFormState,
} from "@/features/scans/targetDescriptor";
import {
  createInitialTargetProfile,
  fullProfileUrl,
  profileToPayload,
  type TargetProfileFormState,
} from "@/features/scans/targetProfile";
import { saveTargetProfile } from "@/shared/ipc/targetProfile";
import { Button, Modal, Select } from "@/shared/components";
import { useToast } from "@/shared/notifications";

const EXAMPLE_CURL = `curl -X POST 'https://api.openai.com/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\
  -H 'Authorization: Bearer YOUR_API_KEY' \\
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{ "role": "user", "content": "Hello" }]
  }'`;

type AddTargetMode = "manual" | "import";

type AddTargetModalProps = {
  open: boolean;
  onClose: () => void;
  defaultProjectId?: string | null;
};

export function AddTargetModal({ open, onClose, defaultProjectId = null }: AddTargetModalProps) {
  const { actions, projects } = useAppStore();
  const { notify } = useToast();
  const [mode, setMode] = useState<AddTargetMode>("manual");
  const [projectId, setProjectId] = useState("");
  const [form, setForm] = useState<TargetFormState>(() => createInitialTargetForm());
  const [curlText, setCurlText] = useState("");
  const [importedProfile, setImportedProfile] = useState<TargetProfileFormState | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setMode("manual");
    setProjectId(defaultProjectId ?? "");
    setForm(createInitialTargetForm());
    setCurlText("");
    setImportedProfile(null);
    setFormError(null);
    setSubmitting(false);
  }, [open, defaultProjectId]);

  function patchForm(patch: Partial<TargetFormState>) {
    setFormError(null);
    setImportedProfile(null);
    setForm((prev) => ({ ...prev, ...patch }));
  }

  function handleModeChange(next: AddTargetMode) {
    if (next === mode || submitting) return;
    setMode(next);
    setFormError(null);
  }

  function handleClose() {
    if (submitting) return;
    onClose();
  }

  function applyCurlToForm(raw: string):
    | { ok: true; form: TargetFormState; profile: TargetProfileFormState }
    | { ok: false; error: string } {
    const result = curlToProfilePatch(raw);
    if (!result.ok) {
      return result;
    }

    const profile = { ...createInitialTargetProfile(), ...result.patch };
    const url = fullProfileUrl(profile);
    let headers: Record<string, string> = {};
    try {
      headers = JSON.parse(profile.headersJson) as Record<string, string>;
    } catch {
      headers = {};
    }

    return {
      ok: true,
      profile,
      form: {
        ...createInitialTargetForm(),
        ...targetFormAuthFromHeaders(headers),
        url,
      },
    };
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!projectId) {
      setFormError("Select a project first.");
      return;
    }

    let nextForm = form;
    let nextProfile = importedProfile;

    if (mode === "import") {
      const applied = applyCurlToForm(curlText);
      if (!applied.ok) {
        setFormError(applied.error);
        return;
      }
      nextForm = applied.form;
      nextProfile = applied.profile;
      setForm(applied.form);
      setImportedProfile(applied.profile);
    }

    const validationError = validateTargetStep(nextForm);
    if (validationError) {
      setFormError(validationError);
      return;
    }

    setSubmitting(true);
    setFormError(null);
    try {
      const descriptor = buildTargetDescriptor(nextForm);
      const name = deriveTargetName(nextForm.url);
      const targetType = nextProfile ? "llm_api" : "web";
      const target = await actions.createTarget(projectId, name, targetType, descriptor);
      if (nextProfile) {
        await saveTargetProfile(target.id, profileToPayload(nextProfile));
      }
      notify(`Target "${name}" added`, "success");
      onClose();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to add target";
      setFormError(message);
      notify(message, "error");
      setSubmitting(false);
    }
  }

  const noProjects = projects.length === 0;
  const canSubmit =
    Boolean(projectId) &&
    (mode === "manual" ? Boolean(form.url.trim()) : Boolean(curlText.trim()));

  return (
    <Modal open={open} title="Add Target" onClose={handleClose} size={mode === "import" ? "medium" : "default"}>
      <form className="project-form" onSubmit={handleSubmit}>
        {noProjects ? (
          <p className="text-muted">Create a project first, then add targets to it.</p>
        ) : (
          <>
            <label className="field">
              <span className="field__label">Project</span>
              <Select value={projectId} onChange={(e) => setProjectId(e.target.value)}>
                <option value="">Select a project…</option>
                {projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name}
                  </option>
                ))}
              </Select>
            </label>

            <div
              className="runtime-route-toggle add-target-modal__mode-toggle"
              role="tablist"
              aria-label="Add target mode"
            >
              <button
                type="button"
                role="tab"
                aria-selected={mode === "manual"}
                className={`runtime-route-toggle__btn${mode === "manual" ? " runtime-route-toggle__btn--active" : ""}`}
                disabled={submitting}
                onClick={() => handleModeChange("manual")}
              >
                Manual
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={mode === "import"}
                className={`runtime-route-toggle__btn${mode === "import" ? " runtime-route-toggle__btn--active" : ""}`}
                disabled={submitting}
                onClick={() => handleModeChange("import")}
              >
                Import
              </button>
            </div>

            {mode === "manual" ? (
              <TargetFormFields form={form} onChange={patchForm} autoFocusUrl={Boolean(projectId)} />
            ) : (
              <div className="import-api-modal">
                <label className="field">
                  <span className="field__label">cURL command</span>
                  <textarea
                    className="input textarea import-api-modal__curl wizard-target-form__mono"
                    rows={12}
                    value={curlText}
                    onChange={(e) => {
                      setFormError(null);
                      setCurlText(e.target.value);
                    }}
                    placeholder={EXAMPLE_CURL}
                    spellCheck={false}
                    autoFocus
                  />
                </label>
              </div>
            )}
          </>
        )}
        {formError && <p className="text-danger">{formError}</p>}
        <div className="project-form__actions">
          <Button variant="ghost" onClick={handleClose} disabled={submitting}>
            Cancel
          </Button>
          <Button
            variant="primary"
            type="submit"
            disabled={submitting || noProjects || !canSubmit}
          >
            {submitting ? "Adding…" : "Add Target"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
