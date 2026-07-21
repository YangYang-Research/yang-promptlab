import { invokeCommand } from "./invoke";

export type ProjectSummaryResponse = {
  source: "ai" | "fallback" | string;
  overview: string;
  highlights: string[];
  generated_at: string;
  target_count: number;
  scan_count: number;
  finding_count: number;
};

export function generateProjectSummary(
  projectId: string,
  force = false,
): Promise<ProjectSummaryResponse> {
  return invokeCommand<ProjectSummaryResponse>("project_summary_generate", {
    request: {
      project_id: projectId,
      force,
    },
  });
}
