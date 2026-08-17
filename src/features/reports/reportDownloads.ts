import { exportReport, generateReport } from "@/shared/ipc";
import type { ReportFormat } from "@/shared/types";

export type ReportExportFormat = Extract<ReportFormat, "html" | "pdf" | "sarif" | "csv">;

const EXPORT_LABELS: Record<ReportExportFormat, string> = {
  html: "HTML",
  pdf: "PDF",
  sarif: "SARIF",
  csv: "CSV",
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

type HtmlReportCandidate = {
  id: string;
  scanId: string | null;
  format: string;
  status: string;
  createdAt: string;
};

export function findLatestHtmlReport<T extends HtmlReportCandidate>(
  reports: T[],
  scanId: string,
): T | null {
  return (
    reports
      .filter(
        (report) =>
          report.scanId === scanId &&
          report.format === "html" &&
          report.status === "completed",
      )
      .sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0] ?? null
  );
}

export type ScanExportRow = {
  scanId: string;
  projectId: string;
  projectName: string;
  scanName: string;
  findingCount: number;
  lastGeneratedAt: string | null;
};

export type ReportScanRow = ScanExportRow & {
  reportId: string;
  lastGeneratedAt: string;
  reportCount: number;
  formats: string[];
};

type BuildReportScanRowsInput = {
  scans: Array<{ id: string; projectId: string; name: string }>;
  findings: Array<{ scanId: string }>;
  reports: Array<{
    id: string;
    scanId: string | null;
    format: string;
    status: string;
    createdAt: string;
  }>;
  projects: Array<{ id: string; name: string }>;
};

/** Scans that already have at least one generated report, newest first. */
export function buildReportScanRows(input: BuildReportScanRowsInput): ReportScanRow[] {
  const findingCounts = new Map<string, number>();
  for (const finding of input.findings) {
    findingCounts.set(finding.scanId, (findingCounts.get(finding.scanId) ?? 0) + 1);
  }

  const projectNames = new Map(input.projects.map((project) => [project.id, project.name]));
  const scansById = new Map(input.scans.map((scan) => [scan.id, scan]));

  const grouped = new Map<
    string,
    {
      latest: string;
      latestReportId: string;
      latestHtmlAt: string | null;
      latestHtmlReportId: string | null;
      count: number;
      formats: Set<string>;
    }
  >();
  for (const report of input.reports) {
    if (!report.scanId) continue;
    const entry = grouped.get(report.scanId) ?? {
      latest: report.createdAt,
      latestReportId: report.id,
      latestHtmlAt: null,
      latestHtmlReportId: null,
      count: 0,
      formats: new Set<string>(),
    };
    entry.count += 1;
    entry.formats.add(report.format.toLowerCase());
    if (report.createdAt > entry.latest) {
      entry.latest = report.createdAt;
      entry.latestReportId = report.id;
    }
    if (
      report.format.toLowerCase() === "html" &&
      report.status === "completed" &&
      (!entry.latestHtmlAt || report.createdAt > entry.latestHtmlAt)
    ) {
      entry.latestHtmlAt = report.createdAt;
      entry.latestHtmlReportId = report.id;
    }
    grouped.set(report.scanId, entry);
  }

  return [...grouped.entries()]
    .map(([scanId, entry]) => {
      const scan = scansById.get(scanId);
      const projectId = scan?.projectId ?? "";
      return {
        scanId,
        reportId: entry.latestHtmlReportId ?? entry.latestReportId,
        projectId,
        projectName: projectNames.get(projectId) ?? "—",
        scanName: scan?.name ?? scanId.slice(0, 8),
        findingCount: findingCounts.get(scanId) ?? 0,
        lastGeneratedAt: entry.latest,
        reportCount: entry.count,
        formats: [...entry.formats].sort(),
      };
    })
    .sort((a, b) => b.lastGeneratedAt.localeCompare(a.lastGeneratedAt));
}

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
