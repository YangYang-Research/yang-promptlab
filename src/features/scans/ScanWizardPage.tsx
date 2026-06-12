import { useMemo, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Button, Card, PageHeader } from "@/shared/components";

const WIZARD_STEPS = [
  "Project",
  "Target & Auth",
  "Discovery",
  "Attack Plan",
  "Submit",
] as const;

function WizardProgress({ current }: { current: number }) {
  return (
    <ol className="wizard-steps">
      {WIZARD_STEPS.map((label, i) => {
        const step = i + 1;
        const state = step < current ? "done" : step === current ? "active" : "upcoming";
        return (
          <li key={label} className={`wizard-steps__item wizard-steps__item--${state}`}>
            <span className="wizard-steps__index">{step}</span>
            <span className="wizard-steps__label">{label}</span>
          </li>
        );
      })}
    </ol>
  );
}

export function ScanWizardPage() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const { projects, loading } = useAppStore();

  const lockedProjectId = params.get("projectId");
  const locked = Boolean(lockedProjectId);

  // Scenario B (no projectId in URL): user must choose a project.
  const [selectedProjectId, setSelectedProjectId] = useState("");

  const lockedProject = useMemo(
    () => (lockedProjectId ? projects.find((p) => p.id === lockedProjectId) ?? null : null),
    [projects, lockedProjectId],
  );

  const activeProjectId = locked ? lockedProjectId : selectedProjectId;
  const canContinue = Boolean(activeProjectId) && (!locked || lockedProject !== null);

  return (
    <div className="page">
      <PageHeader
        title="New Scan"
        description="Configure and launch an AI security scan"
        actions={
          <Button variant="ghost" onClick={() => navigate("/projects")}>
            Cancel
          </Button>
        }
      />

      <WizardProgress current={1} />

      <Card>
        <div className="card__header-row">
          <h3 className="card__title">Step 1 · Project Selection</h3>
          {locked && <Badge variant="muted">Locked</Badge>}
        </div>

        {/* Scenario A: arrived from Projects — project pre-selected and locked. */}
        {locked ? (
          loading && !lockedProject ? (
            <p className="text-muted">Loading project…</p>
          ) : lockedProject ? (
            <div className="wizard-project-locked">
              <div className="field">
                <span className="field__label">Project</span>
                {/* Disabled selector communicates the locked state. */}
                <select className="input" value={lockedProject.id} disabled>
                  <option value={lockedProject.id}>{lockedProject.name}</option>
                </select>
              </div>
              <div className="field">
                <span className="field__label">Description</span>
                <p className="text-muted">
                  {lockedProject.description || "No description"}
                </p>
              </div>
              <p className="text-muted text-sm">
                This scan is locked to <strong>{lockedProject.name}</strong>. To use a different
                project, start the wizard from that project.
              </p>
            </div>
          ) : (
            <div>
              <p className="text-danger">Project not found.</p>
              <p className="text-muted text-sm">
                The project in the link may have been deleted.{" "}
                <Link to="/projects" className="link">
                  Go to Projects
                </Link>
                .
              </p>
            </div>
          )
        ) : (
          /* Scenario B: entered the wizard directly — choose a project. */
          <div className="field">
            <span className="field__label">Project</span>
            <select
              className="input"
              value={selectedProjectId}
              onChange={(e) => setSelectedProjectId(e.target.value)}
            >
              <option value="">{loading ? "Loading…" : "Select a project…"}</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
        )}
      </Card>

      <div className="wizard-footer">
        {/* Step 2+ are intentionally not implemented yet. */}
        <Button variant="primary" disabled={!canContinue}>
          Continue
        </Button>
        <span className="text-muted text-sm">Steps 2–5 are not implemented yet.</span>
      </div>
    </div>
  );
}
