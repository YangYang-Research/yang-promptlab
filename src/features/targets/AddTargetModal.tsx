import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { TargetFormFields } from "@/features/scans/TargetFormFields";
import {
  buildTargetDescriptor,
  createInitialTargetForm,
  deriveTargetName,
  validateTargetStep,
  type TargetFormState,
} from "@/features/scans/targetDescriptor";
import { Button, Modal, Select } from "@/shared/components";
import { useToast } from "@/shared/notifications";

type AddTargetModalProps = {
  open: boolean;
  onClose: () => void;
  defaultProjectId?: string | null;
};

export function AddTargetModal({ open, onClose, defaultProjectId = null }: AddTargetModalProps) {
  const { actions, projects } = useAppStore();
  const { notify } = useToast();
  const [projectId, setProjectId] = useState("");
  const [form, setForm] = useState<TargetFormState>(() => createInitialTargetForm());
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setProjectId(defaultProjectId ?? "");
    setForm(createInitialTargetForm());
    setFormError(null);
    setSubmitting(false);
  }, [open, defaultProjectId]);

  function patchForm(patch: Partial<TargetFormState>) {
    setFormError(null);
    setForm((prev) => ({ ...prev, ...patch }));
  }

  function handleClose() {
    if (submitting) return;
    onClose();
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!projectId) {
      setFormError("Select a project first.");
      return;
    }

    const validationError = validateTargetStep(form);
    if (validationError) {
      setFormError(validationError);
      return;
    }

    setSubmitting(true);
    setFormError(null);
    try {
      const descriptor = buildTargetDescriptor(form);
      const name = deriveTargetName(form.url);
      await actions.createTarget(projectId, name, "web", descriptor);
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
  const canSubmit = Boolean(projectId && form.url.trim());

  return (
    <Modal open={open} title="Add Target" onClose={handleClose}>
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

            <TargetFormFields form={form} onChange={patchForm} autoFocusUrl={Boolean(projectId)} />
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
