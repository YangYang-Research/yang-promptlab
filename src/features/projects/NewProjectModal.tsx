import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Modal } from "@/shared/components";
import { useToast } from "@/shared/notifications";

type NewProjectModalProps = {
  open: boolean;
  onClose: () => void;
};

export function NewProjectModal({ open, onClose }: NewProjectModalProps) {
  const { actions, dispatch } = useAppStore();
  const { notify } = useToast();
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  function reset() {
    setName("");
    setDescription("");
    setFormError(null);
    setSubmitting(false);
  }

  function handleClose() {
    if (submitting) return;
    reset();
    onClose();
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setFormError("Project name is required.");
      return;
    }
    setSubmitting(true);
    setFormError(null);
    try {
      const project = await actions.createProject(trimmed, description.trim() || null);
      notify(`Project "${trimmed}" created`, "success");
      dispatch({ type: "SET_SELECTED_PROJECT", projectId: project.id });
      reset();
      onClose();
      navigate(`/scans/new?projectId=${encodeURIComponent(project.id)}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to create project";
      setFormError(message);
      notify(message, "error");
      setSubmitting(false);
    }
  }

  return (
    <Modal open={open} title="New Project" onClose={handleClose}>
      <form className="project-form" onSubmit={handleSubmit}>
        <label className="field">
          <span className="field__label">Name</span>
          <input
            className="input"
            placeholder="e.g. Acme Chatbot Pentest"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
          />
        </label>
        <label className="field">
          <span className="field__label">Description</span>
          <textarea
            className="input textarea"
            placeholder="Optional summary of the engagement"
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
            {submitting ? "Creating…" : "Create Project"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
