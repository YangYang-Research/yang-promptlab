import { Badge } from "@/shared/components";
import type { Project } from "@/shared/types";

type ProjectStepProps = {
  lockedProjectId: string;
  lockedProject: Project | null;
  resolveError: string | null;
  loading: boolean;
  projects: Project[];
  selectedProjectId: string;
  onSelectProject: (projectId: string) => void;
};

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

export function ProjectStep({
  lockedProjectId,
  lockedProject,
  resolveError,
  loading,
  projects,
  selectedProjectId,
  onSelectProject,
}: ProjectStepProps) {
  const activeProject =
    projects.find((p) => p.id === (lockedProjectId || selectedProjectId)) ?? lockedProject;

  if (lockedProjectId) {
    if (loading && !lockedProject) {
      return <p className="text-muted">Loading project…</p>;
    }
    if (resolveError) {
      return <p className="text-danger">{resolveError}</p>;
    }
    if (lockedProject) {
      return <LockedProjectSelector project={lockedProject} />;
    }
    return <p className="text-danger">Project not found</p>;
  }

  return (
    <>
      <label className="field">
        <span className="field__label">Project</span>
        <select
          className="input"
          value={selectedProjectId}
          onChange={(e) => onSelectProject(e.target.value)}
        >
          <option value="">Select a project…</option>
          {projects.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>
      </label>
      {activeProject?.description && (
        <p className="wizard-project__description text-muted">{activeProject.description}</p>
      )}
    </>
  );
}
