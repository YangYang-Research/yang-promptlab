import { exportReport, generateReport } from "@/shared/ipc";
import type { Finding, Project, Report, ScanRun } from "@/shared/types";

export type ReportExportFormat = "html" | "pdf" | "sarif";

export type ScanExportRow = {
  scanId: string;
  projectId: string;
  projectName: string;
  scanName: string;
  findingCount: number;
  lastGeneratedAt: string | null;
};

export function reportExportLabel(format: ReportExportFormat): string {
  switch (format) {
    case "html":
      return "HTML";
    case "pdf":
      return "PDF";
    case "sarif":
      return "SARIF";
  }
}

export function buildScanExportRows(input: {
  projects: Pick<Project, "id" | "name">[];
  scans: Pick<ScanRun, "id" | "projectId" | "name">[];
  findings: Pick<Finding, "scanId">[];
  reports: Pick<Report, "scanId" | "createdAt">[];
}): ScanExportRow[] {
  const projectNameById = new Map(input.projects.map((project) => [project.id, project.name]));
  const findingCountByScan = new Map<string, number>();

  for (const finding of input.findings) {
    findingCountByScan.set(
      finding.scanId,
      (findingCountByScan.get(finding.scanId) ?? 0) + 1,
    );
  }

  const lastGeneratedByScan = new Map<string, string>();
  for (const report of input.reports) {
    if (!report.scanId) continue;
    const existing = lastGeneratedByScan.get(report.scanId);
    if (!existing || report.createdAt > existing) {
      lastGeneratedByScan.set(report.scanId, report.createdAt);
    }
  }

  return input.scans
    .map((scan) => {
      const findingCount = findingCountByScan.get(scan.id) ?? 0;
      if (findingCount === 0) return null;
      return {
        scanId: scan.id,
        projectId: scan.projectId,
        projectName: projectNameById.get(scan.projectId) ?? scan.projectId,
        scanName: scan.name,
        findingCount,
        lastGeneratedAt: lastGeneratedByScan.get(scan.id) ?? null,
      };
    })
    .filter((row): row is ScanExportRow => row !== null);
}

export async function generateAndExportScanReport(
  projectId: string,
  scanId: string,
  format: ReportExportFormat,
): Promise<string> {
  const report = await generateReport(projectId, scanId, format, "technical");
  return exportReport(report.id);
}

export async function exportStoredReport(reportId: string): Promise<string> {
  return exportReport(reportId);
}
