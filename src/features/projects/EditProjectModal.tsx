import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Modal } from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { Project } from "@/shared/types";

type EditProjectModalProps = {
  open: boolean;
  project: Project | null;
  onClose: () => void;
  onSaved?: (project: Project) => void;
};

export function EditProjectModal({ open, project, onClose, onSaved }: EditProjectModalProps) {
  const { actions } = useAppStore();
  const { notify } = useToast();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !project) return;
    setName(project.name);
    setDescription(project.description);
    setFormError(null);
    setSubmitting(false);
  }, [open, project]);

  function handleClose() {
    if (submitting) return;
    onClose();
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!project) return;
    const trimmed = name.trim();
    if (!trimmed) {
      setFormError("Project name is required.");
      return;
    }

    setSubmitting(true);
    setFormError(null);
    try {
      const updated = await actions.updateProject(
        project.id,
        trimmed,
        description.trim() || null,
      );
      notify(`Project "${trimmed}" updated`, "success");
      onSaved?.(updated);
      onClose();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to update project";
      setFormError(message);
      notify(message, "error");
      setSubmitting(false);
    }
  }

  return (
    <Modal open={open} title="Edit Project" onClose={handleClose}>
      <form className="project-form" onSubmit={handleSubmit}>
        <label className="field">
          <span className="field__label">Name</span>
          <input
            className="input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
          />
        </label>
        <label className="field">
          <span className="field__label">Description</span>
          <textarea
            className="input textarea"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
          />
        </label>
        {formError && <p className="text-danger">{formError}</p>}
        <div className="project-form__actions">
          <Button variant="ghost" onClick={handleClose} disabled={submitting}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" disabled={submitting || !name.trim()}>
            {submitting ? "Saving…" : "Save Changes"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
