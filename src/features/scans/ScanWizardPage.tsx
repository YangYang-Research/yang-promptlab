import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { mapProjects } from "@/app/store/mappers";
import { Badge, Card, PageHeader } from "@/shared/components";
import { getProject } from "@/shared/ipc";
import type { Project, Target } from "@/shared/types";

import type { AttackPlanConfig } from "./attackProfiles";
import { AttackPlanStep } from "./steps/AttackPlanStep";
import { DiscoveryStep, type DiscoverySelection } from "./steps/DiscoveryStep";
import { SubmitStep } from "./steps/SubmitStep";
import { TargetStep } from "./steps/TargetStep";

function LockedProjectSelector({ project }: { project: Project }) {
  return (
    <div className="wizard-project wizard-project--locked">
      <div className="wizard-project__header">
        <span className="field__label">Project</span>
        <Badge variant="muted">Locked</Badge>
      </div>
      <input className="input" value={project.name} readOnly disabled aria-readonly />
      {project.description ? (
        <p className="wizard-project__description">{project.description}</p>
      ) : (
        <p className="wizard-project__description text-muted">No description</p>
      )}
    </div>
  );
}

function ProjectDropdown({
  projects,
  value,
  onChange,
}: {
  projects: Project[];
  value: string;
  onChange: (projectId: string) => void;
}) {
  return (
    <label className="field">
      <span className="field__label">Project</span>
      <select
        className="input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value="">Select a project…</option>
        {projects.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function ScanWizardPage() {
  const [searchParams] = useSearchParams();
  const lockedProjectId = searchParams.get("projectId")?.trim() ?? "";
  const { projects, loading, error, dispatch } = useAppStore();
  const [resolvedProject, setResolvedProject] = useState<Project | null>(null);
  const [resolveError, setResolveError] = useState<string | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [savedTarget, setSavedTarget] = useState<Target | null>(null);
  const [discoverySelection, setDiscoverySelection] = useState<DiscoverySelection>({
    scanId: null,
    selectedCount: 0,
    selectedEndpointIds: [],
  });
  const [attackPlan, setAttackPlan] = useState<AttackPlanConfig | null>(null);

  const storeProject = lockedProjectId
    ? projects.find((p) => p.id === lockedProjectId)
    : null;
  const lockedProject = storeProject ?? resolvedProject;

  useEffect(() => {
    if (!lockedProjectId) {
      setResolvedProject(null);
      setResolveError(null);
      return;
    }

    if (storeProject) {
      setResolvedProject(null);
      setResolveError(null);
      dispatch({ type: "SET_SELECTED_PROJECT", projectId: storeProject.id });
      return;
    }

    if (loading) return;

    let cancelled = false;
    setResolveError(null);

    void getProject(lockedProjectId)
      .then((dto) => {
        if (cancelled) return;
        const project = mapProjects([dto], [], [])[0];
        setResolvedProject(project);
        dispatch({ type: "SET_SELECTED_PROJECT", projectId: project.id });
      })
      .catch((err) => {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : "Project not found";
        setResolveError(message);
        setResolvedProject(null);
      });

    return () => {
      cancelled = true;
    };
  }, [lockedProjectId, storeProject, loading, dispatch]);

  const activeProjectId = lockedProjectId || selectedProjectId;
  const activeProject = projects.find((p) => p.id === activeProjectId) ?? lockedProject;

  return (
    <div className="page">
      <PageHeader
        title="New Scan"
        description="Configure a new security scan"
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <Card>
        <div className="wizard-step">
          <div className="wizard-step__heading">
            <span className="wizard-step__number">1</span>
            <div>
              <h3 className="wizard-step__title">Project</h3>
              <p className="wizard-step__hint text-muted">
                Choose the project this scan belongs to
              </p>
            </div>
          </div>

          {lockedProjectId ? (
            loading && !lockedProject ? (
              <p className="text-muted">Loading project…</p>
            ) : resolveError ? (
              <p className="text-danger">{resolveError}</p>
            ) : lockedProject ? (
              <LockedProjectSelector project={lockedProject} />
            ) : (
              <p className="text-danger">Project not found</p>
            )
          ) : (
            <ProjectDropdown
              projects={projects}
              value={selectedProjectId}
              onChange={(id) => {
                setSelectedProjectId(id);
                dispatch({ type: "SET_SELECTED_PROJECT", projectId: id || null });
              }}
            />
          )}

          {activeProject && !lockedProjectId && (
            <p className="wizard-project__description text-muted">{activeProject.description}</p>
          )}
        </div>
      </Card>

      {activeProjectId ? (
        <Card>
          <TargetStep projectId={activeProjectId} onTargetSaved={setSavedTarget} />
        </Card>
      ) : (
        <Card>
          <div className="wizard-step wizard-step--disabled">
            <div className="wizard-step__heading">
              <span className="wizard-step__number">2</span>
              <div>
                <h3 className="wizard-step__title">Target &amp; authentication</h3>
                <p className="wizard-step__hint text-muted">
                  Select a project in step 1 to configure the target
                </p>
              </div>
            </div>
          </div>
        </Card>
      )}

      {savedTarget ? (
        <Card>
          <DiscoveryStep target={savedTarget} onSelectionChange={setDiscoverySelection} />
        </Card>
      ) : (
        <Card>
          <div className="wizard-step wizard-step--disabled">
            <div className="wizard-step__heading">
              <span className="wizard-step__number">3</span>
              <div>
                <h3 className="wizard-step__title">Discovery</h3>
                <p className="wizard-step__hint text-muted">
                  Save a target in step 2 to run discovery
                </p>
              </div>
            </div>
          </div>
        </Card>
      )}

      {discoverySelection.selectedCount > 0 ? (
        <Card>
          <AttackPlanStep
            selectedEndpointCount={discoverySelection.selectedCount}
            onPlanChange={setAttackPlan}
          />
        </Card>
      ) : (
        <Card>
          <div className="wizard-step wizard-step--disabled">
            <div className="wizard-step__heading">
              <span className="wizard-step__number">4</span>
              <div>
                <h3 className="wizard-step__title">Attack planning</h3>
                <p className="wizard-step__hint text-muted">
                  {savedTarget
                    ? "Select at least one endpoint in step 3 to configure the attack profile"
                    : "Complete discovery in step 3 to choose an attack profile"}
                </p>
              </div>
            </div>
          </div>
        </Card>
      )}

      {activeProjectId &&
      savedTarget &&
      attackPlan &&
      attackPlan.categories.length > 0 &&
      discoverySelection.selectedEndpointIds.length > 0 ? (
        <Card>
          <SubmitStep
            projectId={activeProjectId}
            target={savedTarget}
            endpointIds={discoverySelection.selectedEndpointIds}
            attackPlan={attackPlan}
          />
        </Card>
      ) : (
        <Card>
          <div className="wizard-step wizard-step--disabled">
            <div className="wizard-step__heading">
              <span className="wizard-step__number">5</span>
              <div>
                <h3 className="wizard-step__title">Start scan</h3>
                <p className="wizard-step__hint text-muted">
                  {discoverySelection.selectedCount > 0
                    ? "Choose an attack profile in step 4 to start the scan"
                    : "Complete steps 1–4 to submit the scan"}
                </p>
              </div>
            </div>
          </div>
        </Card>
      )}
    </div>
  );
}
