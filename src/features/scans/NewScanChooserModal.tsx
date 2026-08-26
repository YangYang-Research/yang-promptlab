import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Modal, Select } from "@/shared/components";
import { useToast } from "@/shared/notifications";

import {
  clearScanConfigImport,
  parseScanConfigExport,
  stashScanConfigImport,
} from "./scanConfigExport";
import { buildScanWizardUrl } from "./wizardState";

type ChooserView = "choose" | "import";

type NewScanChooserModalProps = {
  open: boolean;
  onClose: () => void;
  projectId?: string | null;
};

function newScanWizardPath(projectId?: string | null): string {
  const id = projectId?.trim();
  return id ? buildScanWizardUrl(id) : "/scans/new";
}

export function NewScanChooserModal({
  open,
  onClose,
  projectId = null,
}: NewScanChooserModalProps) {
  const navigate = useNavigate();
  const { projects } = useAppStore();
  const { notify } = useToast();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [view, setView] = useState<ChooserView>("choose");
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [jsonText, setJsonText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) return;
    setView("choose");
    setSelectedProjectId(projectId ?? "");
    setJsonText("");
    setError(null);
    setSubmitting(false);
  }, [open, projectId]);

  function handleClose() {
    if (submitting) return;
    onClose();
  }

  function goManual() {
    clearScanConfigImport();
    onClose();
    navigate(newScanWizardPath(projectId));
  }

  function goImportView() {
    setView("import");
    setError(null);
  }

  async function handleFileChange(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      const text = await file.text();
      setJsonText(text);
      setError(null);
    } catch {
      setError("Failed to read file.");
    }
  }

  function handleImportSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!selectedProjectId.trim()) {
      setError("Select a project first.");
      return;
    }
    const raw = jsonText.trim();
    if (!raw) {
      setError("Paste scan config JSON or choose a file.");
      return;
    }

    const parsed = parseScanConfigExport(raw);
    if (!parsed.ok) {
      setError(parsed.error);
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      stashScanConfigImport(parsed.config);
      onClose();
      navigate(newScanWizardPath(selectedProjectId));
      notify("Starting import…", "info");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to import scan config";
      setError(message);
      notify(message, "error");
      setSubmitting(false);
    }
  }

  const noProjects = projects.length === 0;
  const canImport = Boolean(selectedProjectId.trim()) && Boolean(jsonText.trim());

  return (
    <Modal
      open={open}
      title={view === "choose" ? "New Scan" : "Import Scan"}
      onClose={handleClose}
      size={view === "import" ? "medium" : "default"}
    >
      {view === "choose" ? (
        <div className="new-scan-chooser">
          {noProjects ? (
            <>
              <p className="text-muted new-scan-chooser__lead">
                Create a project first, then start a scan with manual setup or import.
              </p>
              <div className="project-form__actions">
                <Button type="button" variant="ghost" onClick={handleClose}>
                  Cancel
                </Button>
                <Button
                  type="button"
                  variant="primary"
                  onClick={() => {
                    onClose();
                    navigate("/projects", { state: { openNewProject: true } });
                  }}
                >
                  Create project
                </Button>
              </div>
            </>
          ) : (
            <>
              <p className="text-muted new-scan-chooser__lead">
                Configure a scan manually or import a previously exported scan config.
              </p>
              <div className="new-scan-chooser__options" role="group" aria-label="New scan options">
                <button type="button" className="new-scan-chooser__option" onClick={goManual}>
                  <span className="new-scan-chooser__option-title">Manual Setup</span>
                  <span className="new-scan-chooser__option-desc">
                    Walk through the scan wizard and configure endpoint, auth, and attack plan.
                  </span>
                </button>
                <button type="button" className="new-scan-chooser__option" onClick={goImportView}>
                  <span className="new-scan-chooser__option-title">Import Scan</span>
                  <span className="new-scan-chooser__option-desc">
                    Load a scan-config JSON exported from a completed scan.
                  </span>
                </button>
              </div>
              <div className="project-form__actions">
                <Button type="button" variant="ghost" onClick={handleClose}>
                  Cancel
                </Button>
              </div>
            </>
          )}
        </div>
      ) : (
        <form className="project-form" onSubmit={handleImportSubmit}>
          {noProjects ? (
            <p className="text-muted">Create a project first, then import a scan into it.</p>
          ) : (
            <>
              <label className="field">
                <span className="field__label">Project</span>
                <Select
                  value={selectedProjectId}
                  onChange={(e) => {
                    setSelectedProjectId(e.target.value);
                    setError(null);
                  }}
                  disabled={submitting}
                >
                  <option value="">Select a project…</option>
                  {projects.map((project) => (
                    <option key={project.id} value={project.id}>
                      {project.name}
                    </option>
                  ))}
                </Select>
              </label>
              <p className="text-muted">
                Paste exported scan config JSON, or choose a <code>.json</code> file.
              </p>
              <label className="field">
                <span className="field__label">Scan config JSON</span>
                <textarea
                  className="input textarea wizard-target-form__mono"
                  rows={12}
                  value={jsonText}
                  onChange={(e) => {
                    setJsonText(e.target.value);
                    setError(null);
                  }}
                  placeholder='{ "format": "promptlab.scan_config", "version": 1, ... }'
                  disabled={submitting}
                  spellCheck={false}
                />
              </label>
              <div className="add-target-modal__import-row">
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="application/json,.json"
                  hidden
                  onChange={(e) => void handleFileChange(e)}
                />
                <Button
                  type="button"
                  variant="secondary"
                  disabled={submitting}
                  onClick={() => fileInputRef.current?.click()}
                >
                  Choose file…
                </Button>
              </div>
            </>
          )}
          {error && <p className="text-danger">{error}</p>}
          <div className="project-form__actions new-scan-chooser__actions">
            <Button
              type="button"
              variant="ghost"
              disabled={submitting}
              onClick={() => setView("choose")}
            >
              Back
            </Button>
            {!noProjects && (
              <Button type="submit" variant="primary" disabled={submitting || !canImport}>
                {submitting ? "Importing…" : "Import & continue"}
              </Button>
            )}
          </div>
        </form>
      )}
    </Modal>
  );
}
