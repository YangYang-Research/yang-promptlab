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
    projects.find((project) => project.id === (lockedProjectId || selectedProjectId)) ??
    lockedProject;

  if (lockedProjectId) {
    if (loading && !lockedProject) {
      return <p className="text-muted">Loading project…</p>;
    }
    if (resolveError) {
      return <p className="text-danger">{resolveError}</p>;
    }
    if (lockedProject) {
      return (
        <label className="field">
          <span className="field__label">Project</span>
          <select className="input" value={lockedProject.id} disabled aria-readonly>
            <option value={lockedProject.id}>{lockedProject.name}</option>
          </select>
          {lockedProject.description ? (
            <p className="wizard-project__description text-muted">{lockedProject.description}</p>
          ) : null}
        </label>
      );
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
