import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Modal, Select } from "@/shared/components";
import { importFindingsSarif } from "@/shared/ipc";
import { pickSarifImportFile } from "@/shared/ipc/dialog";
import { useToast } from "@/shared/notifications";

type ImportSarifModalProps = {
  open: boolean;
  onClose: () => void;
};

export function ImportSarifModal({ open, onClose }: ImportSarifModalProps) {
  const { projects, actions } = useAppStore();
  const { notify } = useToast();
  const [projectId, setProjectId] = useState("");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setProjectId("");
    setFilePath(null);
    setError(null);
    setSubmitting(false);
  }, [open]);

  async function handlePickFile() {
    setError(null);
    try {
      const selected = await pickSarifImportFile();
      if (selected) setFilePath(selected);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to open file picker");
    }
  }

  async function handleImport() {
    if (!filePath) {
      setError("Choose a SARIF file");
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      const result = await importFindingsSarif(filePath, projectId || null);
      await actions.refresh();
      notify(
        `Imported ${result.imported_count} finding${result.imported_count === 1 ? "" : "s"} from SARIF`,
        "success",
      );
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "SARIF import failed");
    } finally {
      setSubmitting(false);
    }
  }

  const fileLabel = filePath
    ? filePath.split(/[/\\]/).pop() ?? filePath
    : "No file selected";

  return (
    <Modal
      open={open}
      title="Import Finding"
      onClose={() => {
        if (!submitting) onClose();
      }}
      footer={
        <div className="project-form__actions">
          <Button variant="ghost" disabled={submitting} onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={submitting || !filePath}
            onClick={() => void handleImport()}
          >
            {submitting ? "Importing…" : "Import"}
          </Button>
        </div>
      }
    >
      <div className="import-sarif-modal">
        <p className="text-muted text-sm">
          Import findings from a PromptLab SARIF export. Scan and project are read from the file
          (<code>runs[].properties</code>). Use destination project only when the SARIF project is
          missing or from another workspace.
        </p>

        <div className="field">
          <span className="field__label">SARIF file</span>
          <div className="import-sarif-modal__file-row">
            <Button
              type="button"
              variant="secondary"
              disabled={submitting}
              onClick={() => void handlePickFile()}
            >
              Choose file…
            </Button>
            <span
              className="text-sm text-muted mono import-sarif-modal__file-name"
              title={filePath ?? undefined}
            >
              {fileLabel}
            </span>
          </div>
        </div>

        <label className="field">
          <span className="field__label">Destination project (fallback)</span>
          <Select
            value={projectId}
            disabled={submitting || projects.length === 0}
            onChange={(e) => {
              setProjectId(e.target.value);
              setError(null);
            }}
          >
            <option value="">Use project from SARIF</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </Select>
        </label>

        {error && <p className="text-danger">{error}</p>}
      </div>
    </Modal>
  );
}
