import type { Finding, Project, ScanRun, Severity } from "@/shared/types";

export type FindingFilters = {
  searchQuery: string;
  projectId: string | null;
  scanId: string | null;
  severity: Severity | null;
  status: Finding["status"] | null;
};

export function filterFindings(
  findings: Finding[],
  filters: FindingFilters,
  projects: Project[],
  scans: ScanRun[],
): Finding[] {
  const query = filters.searchQuery.toLowerCase().trim();

  return findings.filter((finding) => {
    if (filters.projectId && finding.projectId !== filters.projectId) {
      return false;
    }
    if (filters.scanId && finding.scanId !== filters.scanId) {
      return false;
    }
    if (filters.severity && finding.severity !== filters.severity) {
      return false;
    }
    if (filters.status && finding.status !== filters.status) {
      return false;
    }
    if (!query) {
      return true;
    }

    const project = projects.find((p) => p.id === finding.projectId);
    const scan = scans.find((s) => s.id === finding.scanId);

    return (
      finding.title.toLowerCase().includes(query) ||
      finding.description.toLowerCase().includes(query) ||
      finding.targetName.toLowerCase().includes(query) ||
      finding.category.toLowerCase().includes(query) ||
      finding.status.toLowerCase().includes(query) ||
      project?.name.toLowerCase().includes(query) ||
      scan?.name.toLowerCase().includes(query) ||
      finding.scanId.toLowerCase().includes(query)
    );
  });
}
