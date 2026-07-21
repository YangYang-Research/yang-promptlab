import type { Finding, Project, ScanRun, Severity } from "@/shared/types";

export type FindingFilters = {
  searchQuery: string;
  projectIds: string[];
  scanIds: string[];
  severities: Severity[];
  statuses: Finding["status"][];
};

export function filterFindings(
  findings: Finding[],
  filters: FindingFilters,
  projects: Project[],
  scans: ScanRun[],
): Finding[] {
  const query = filters.searchQuery.toLowerCase().trim();
  const projectIds = new Set(filters.projectIds);
  const scanIds = new Set(filters.scanIds);
  const severities = new Set(filters.severities);
  const statuses = new Set(filters.statuses);

  return findings.filter((finding) => {
    if (projectIds.size > 0 && !projectIds.has(finding.projectId)) {
      return false;
    }
    if (scanIds.size > 0 && !scanIds.has(finding.scanId)) {
      return false;
    }
    if (severities.size > 0 && !severities.has(finding.severity)) {
      return false;
    }
    if (statuses.size > 0 && !statuses.has(finding.status)) {
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
