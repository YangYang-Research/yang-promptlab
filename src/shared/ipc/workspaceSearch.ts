import { invokeCommand } from "./invoke";

export type WorkspaceSearchKind =
  | "project"
  | "target"
  | "scan"
  | "finding"
  | "report"
  | "technique";

export type WorkspaceSearchHit = {
  id: string;
  kind: WorkspaceSearchKind | string;
  title: string;
  subtitle: string;
  to: string;
};

export function searchWorkspace(query: string): Promise<WorkspaceSearchHit[]> {
  return invokeCommand<WorkspaceSearchHit[]>("workspace_search", { query });
}
