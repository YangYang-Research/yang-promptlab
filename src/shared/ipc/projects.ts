import { invokeCommand } from "./invoke";

/** Mirror of Rust `ProjectDto` (timestamps are RFC 3339 strings). */
export type ProjectDto = {
  id: string;
  name: string;
  description: string | null;
  /** Persisted AI/fallback project summary JSON when present. */
  summary?: Record<string, unknown> | null;
  /** Persisted health score 0–100; `null` when not yet scored. */
  health_score?: number | null;
  created_at: string;
  updated_at: string;
};

export const listProjects = () => invokeCommand<ProjectDto[]>("project_list");

export const createProject = (name: string, description?: string | null) =>
  invokeCommand<ProjectDto>("project_create", { name, description: description ?? null });

export const getProject = (id: string) =>
  invokeCommand<ProjectDto>("project_get", { id });

export const updateProject = (
  id: string,
  name?: string | null,
  description?: string | null,
) =>
  invokeCommand<ProjectDto>("project_update", {
    id,
    name: name ?? null,
    description: description ?? null,
  });

export const deleteProject = (id: string) =>
  invokeCommand<null>("project_delete", { id });
