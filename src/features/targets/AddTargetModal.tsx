import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Modal } from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { TargetType } from "@/shared/types";

type AddTargetModalProps = {
  open: boolean;
  onClose: () => void;
};

const TARGET_TYPES: TargetType[] = ["llm", "api", "web", "mobile"];

export function AddTargetModal({ open, onClose }: AddTargetModalProps) {
  const { actions, projects, ui } = useAppStore();
  const { notify } = useToast();
  const [projectId, setProjectId] = useState("");
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [targetType, setTargetType] = useState<TargetType>("llm");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    const fallback = ui.selectedProjectId ?? projects[0]?.id ?? "";
    setProjectId((current) => current || fallback);
  }, [open, ui.selectedProjectId, projects]);

  function reset() {
    setName("");
    setUrl("");
    setTargetType("llm");
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
    const trimmedUrl = url.trim();
    if (!projectId) {
      setFormError("Select a project first.");
      return;
    }
    if (!trimmedUrl) {
      setFormError("Target URL is required.");
      return;
    }
    const trimmedName = name.trim() || trimmedUrl;
    setSubmitting(true);
    setFormError(null);
    try {
      await actions.createTarget(projectId, trimmedName, targetType, { url: trimmedUrl });
      notify(`Target "${trimmedName}" added`, "success");
      reset();
      onClose();
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to add target";
      setFormError(message);
      notify(message, "error");
      setSubmitting(false);
    }
  }

  const noProjects = projects.length === 0;

  return (
    <Modal open={open} title="Add Target" onClose={handleClose}>
      <form className="project-form" onSubmit={handleSubmit}>
        {noProjects ? (
          <p className="text-muted">Create a project first, then add targets to it.</p>
        ) : (
          <>
            <label className="field">
              <span className="field__label">Project</span>
              <select
                className="input"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
              >
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span className="field__label">Target URL</span>
              <input
                className="input"
                placeholder="https://api.example.com/v1/chat/completions"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                autoFocus
              />
            </label>
            <label className="field">
              <span className="field__label">Name</span>
              <input
                className="input"
                placeholder="Optional — defaults to the URL"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="field">
              <span className="field__label">Type</span>
              <select
                className="input"
                value={targetType}
                onChange={(e) => setTargetType(e.target.value as TargetType)}
              >
                {TARGET_TYPES.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </label>
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
            disabled={submitting || noProjects || !url.trim()}
          >
            {submitting ? "Adding…" : "Add Target"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
