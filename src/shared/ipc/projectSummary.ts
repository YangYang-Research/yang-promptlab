import { invokeCommand } from "./invoke";

export type ProjectSummaryFailedScanDto = {
  scan_id: string;
  scan_name: string;
  status: string;
  target_id?: string | null;
  target_name?: string | null;
  /** Full endpoint URL for the target (preferred display label). */
  target_url?: string | null;
};

export type ProjectSummaryActionDto = {
  title: string;
  description: string;
  action: string;
  scan_id: string;
  target_id?: string | null;
};

export type ProjectSummaryResponse = {
  source: "ai" | "fallback" | string;
  overview: string;
  highlights: string[];
  failed_scans?: ProjectSummaryFailedScanDto[];
  actions?: ProjectSummaryActionDto[];
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
