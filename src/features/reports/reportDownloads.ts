import { exportReport, generateReport } from "@/shared/ipc";
import type { ReportFormat } from "@/shared/types";

export type ReportExportFormat = Extract<ReportFormat, "html" | "pdf" | "sarif">;

const EXPORT_LABELS: Record<ReportExportFormat, string> = {
  html: "HTML",
  pdf: "PDF",
  sarif: "SARIF",
};

export function reportExportLabel(format: ReportExportFormat): string {
  return EXPORT_LABELS[format];
}

export async function generateAndExportScanReport(
  projectId: string,
  scanId: string,
  format: ReportExportFormat,
): Promise<string> {
  const report = await generateReport(projectId, scanId, format, "technical");
  if (report.status !== "completed") {
    throw new Error(`Report generation failed (${report.status})`);
  }
  return exportReport(report.id);
}

export async function exportStoredReport(reportId: string): Promise<string> {
  return exportReport(reportId);
}

export type ScanExportRow = {
  scanId: string;
  projectId: string;
  projectName: string;
  scanName: string;
  findingCount: number;
  lastGeneratedAt: string | null;
};

type BuildScanExportRowsInput = {
  scans: Array<{ id: string; projectId: string; name: string }>;
  findings: Array<{ scanId: string }>;
  reports: Array<{ scanId: string | null; createdAt: string }>;
  projects: Array<{ id: string; name: string }>;
};

export function buildScanExportRows(input: BuildScanExportRowsInput): ScanExportRow[] {
  const findingCounts = new Map<string, number>();
  for (const finding of input.findings) {
    findingCounts.set(finding.scanId, (findingCounts.get(finding.scanId) ?? 0) + 1);
  }

  const lastGenerated = new Map<string, string>();
  for (const report of input.reports) {
    if (!report.scanId) continue;
    const existing = lastGenerated.get(report.scanId);
    if (!existing || report.createdAt > existing) {
      lastGenerated.set(report.scanId, report.createdAt);
    }
  }

  const projectNames = new Map(input.projects.map((project) => [project.id, project.name]));

  return input.scans
    .filter((scan) => (findingCounts.get(scan.id) ?? 0) > 0)
    .map((scan) => ({
      scanId: scan.id,
      projectId: scan.projectId,
      projectName: projectNames.get(scan.projectId) ?? "—",
      scanName: scan.name,
      findingCount: findingCounts.get(scan.id) ?? 0,
      lastGeneratedAt: lastGenerated.get(scan.id) ?? null,
    }))
    .sort((a, b) => {
      const aTime = a.lastGeneratedAt ?? "";
      const bTime = b.lastGeneratedAt ?? "";
      return bTime.localeCompare(aTime);
    });
}
