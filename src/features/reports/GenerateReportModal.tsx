import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Modal } from "@/shared/components";
import { useToast } from "@/shared/notifications";

type GenerateReportModalProps = {
  open: boolean;
  onClose: () => void;
};

const KINDS = [
  { id: "technical", label: "Technical" },
  { id: "executive", label: "Executive" },
  { id: "compliance", label: "Compliance" },
] as const;

export function GenerateReportModal({ open, onClose }: GenerateReportModalProps) {
  const { actions, projects, scans, findings, ui } = useAppStore();
  const { notify } = useToast();
  const [projectId, setProjectId] = useState("");
  const [scanId, setScanId] = useState("");
  const [kind, setKind] = useState("technical");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setProjectId((cur) => cur || ui.selectedProjectId || projects[0]?.id || "");
  }, [open, ui.selectedProjectId, projects]);

  const projectScans = useMemo(
    () => scans.filter((s) => s.projectId === projectId),
    [scans, projectId],
  );

  // Default the scan selection to one that has findings, if any.
  useEffect(() => {
    if (!open) return;
    const withFindings = projectScans.find((s) => findings.some((f) => f.scanId === s.id));
    setScanId((cur) => {
      if (cur && projectScans.some((s) => s.id === cur)) return cur;
      return withFindings?.id ?? projectScans[0]?.id ?? "";
    });
  }, [open, projectScans, findings]);

  const scanFindingCount = (id: string) => findings.filter((f) => f.scanId === id).length;

  function handleClose() {
    if (submitting) return;
    setFormError(null);
    onClose();
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!projectId || !scanId) {
      setFormError("Select a project and a scan.");
      return;
    }
    setSubmitting(true);
    setFormError(null);
    try {
      await actions.generateReport(projectId, scanId, "html", kind);
      notify("HTML report generated", "success");
      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to generate report";
      setFormError(message);
      notify(message, "error");
    } finally {
      setSubmitting(false);
    }
  }

  const noScans = projectScans.length === 0;

  return (
    <Modal open={open} title="Generate HTML Report" onClose={handleClose}>
      <form className="project-form" onSubmit={handleSubmit}>
        <label className="field">
          <span className="field__label">Project</span>
          <select className="input" value={projectId} onChange={(e) => setProjectId(e.target.value)}>
            {projects.length === 0 && <option value="">No projects</option>}
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field__label">Scan</span>
          <select
            className="input"
            value={scanId}
            onChange={(e) => setScanId(e.target.value)}
            disabled={noScans}
          >
            {noScans && <option value="">No scans for this project</option>}
            {projectScans.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name} — {scanFindingCount(s.id)} finding(s)
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field__label">Report type</span>
          <select className="input" value={kind} onChange={(e) => setKind(e.target.value)}>
            {KINDS.map((k) => (
              <option key={k.id} value={k.id}>
                {k.label}
              </option>
            ))}
          </select>
        </label>
        {formError && <p className="text-danger">{formError}</p>}
        <div className="project-form__actions">
          <Button variant="ghost" onClick={handleClose} disabled={submitting}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" disabled={submitting || noScans}>
            {submitting ? "Generating…" : "Generate Report"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
