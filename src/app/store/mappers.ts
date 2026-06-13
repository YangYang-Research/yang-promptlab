import type {
  EndpointDto,
  FindingDto,
  ProjectDto,
  ReportDto,
  ScanDto,
  TargetDto,
} from "@/shared/ipc";
import type {
  DiscoveredEndpoint,
  Finding,
  JobStatus,
  Project,
  Report,
  ReportFormat,
  ScanRun,
  Severity,
  Target,
  TargetType,
} from "@/shared/types";

const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];
const FINDING_STATUSES: Finding["status"][] = ["open", "confirmed", "false_positive", "fixed"];
const REPORT_FORMATS: ReportFormat[] = ["html", "pdf", "json", "sarif", "markdown"];

function coerceSeverity(value: string): Severity {
  const v = value.toLowerCase();
  return (SEVERITIES as string[]).includes(v) ? (v as Severity) : "info";
}

function coerceFindingStatus(value: string): Finding["status"] {
  const v = value.toLowerCase();
  return (FINDING_STATUSES as string[]).includes(v) ? (v as Finding["status"]) : "open";
}

function coerceTargetType(value: string): TargetType {
  const v = value.toLowerCase();
  if (v.includes("llm")) return "llm";
  if (v.includes("mobile")) return "mobile";
  if (v.includes("web")) return "web";
  return "api";
}

function coerceReportFormat(value: string): ReportFormat {
  const v = value.toLowerCase();
  return (REPORT_FORMATS as string[]).includes(v) ? (v as ReportFormat) : "html";
}

function coerceJobStatus(value: string, fallback: JobStatus = "completed"): JobStatus {
  switch (value.toLowerCase()) {
    case "pending":
    case "running":
    case "paused":
    case "completed":
    case "failed":
    case "cancelled":
      return value.toLowerCase() as JobStatus;
    default:
      return fallback;
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

import { extractAuthType, extractTargetUrl } from "@/features/scans/scanDetailsHelpers";

function extractConfidence(evidence: unknown): number {
  const obj = asRecord(evidence);
  if (obj && typeof obj.confidence === "number") {
    return Math.max(0, Math.min(1, obj.confidence as number));
  }
  return 0;
}

function extractVerdict(evidence: unknown): Finding["verdict"] {
  const obj = asRecord(evidence);
  if (!obj) return null;
  const judge = asRecord(obj.judge);
  if (judge && typeof judge.vulnerable === "boolean") {
    return judge.vulnerable ? "vulnerable" : "not_vulnerable";
  }
  if (typeof obj.verdict === "string") {
    return obj.verdict === "vulnerable" ? "vulnerable" : "not_vulnerable";
  }
  return null;
}

export function mapProjects(
  projects: ProjectDto[],
  targets: TargetDto[],
  findings: FindingDto[],
): Project[] {
  const targetCounts = countBy(targets, (t) => t.project_id);
  const findingCounts = countBy(findings, (f) => f.project_id);
  return projects.map((p) => ({
    id: p.id,
    name: p.name,
    description: p.description ?? "",
    status: "active",
    createdAt: p.created_at,
    updatedAt: p.updated_at,
    targetCount: targetCounts.get(p.id) ?? 0,
    findingCount: findingCounts.get(p.id) ?? 0,
    owner: "",
  }));
}

export function mapTargets(targets: TargetDto[]): Target[] {
  return targets.map((t) => ({
    id: t.id,
    projectId: t.project_id,
    name: t.name,
    url: extractTargetUrl(t.descriptor),
    type: coerceTargetType(t.target_type),
    status: "pending",
    lastScanAt: null,
    fingerprint: null,
    tags: [],
    authType: extractAuthType(t.descriptor),
  }));
}

export function mapFindings(findings: FindingDto[], targets: TargetDto[]): Finding[] {
  const targetNames = new Map(targets.map((t) => [t.id, t.name]));
  return findings.map((f) => ({
    id: f.id,
    scanId: f.scan_id,
    projectId: f.project_id,
    targetId: f.target_id ?? "",
    targetName: f.target_id ? targetNames.get(f.target_id) ?? "" : "",
    title: f.title,
    description: f.description ?? "",
    severity: coerceSeverity(f.severity),
    category: f.category ?? "general",
    status: coerceFindingStatus(f.status),
    confidence: extractConfidence(f.evidence),
    verdict: extractVerdict(f.evidence),
    discoveredAt: f.created_at,
  }));
}

export function mapScans(scans: ScanDto[]): ScanRun[] {
  return scans.map((s) => ({
    id: s.id,
    projectId: s.project_id,
    targetId: s.target_id,
    name: s.name,
    status: coerceJobStatus(s.status, "pending"),
    startedAt: s.started_at,
    completedAt: s.completed_at,
    createdAt: s.created_at,
  }));
}

export function mapEndpoints(endpoints: EndpointDto[]): DiscoveredEndpoint[] {
  return endpoints.map((e) => ({
    id: e.id,
    scanId: e.scan_id,
    targetId: e.target_id,
    url: e.url,
    kind: e.kind,
    method: e.method,
    confidence: e.confidence,
    evidence: e.evidence,
    sourceUrl: e.source_url,
    discoveredAt: e.discovered_at,
    fingerprint: e.fingerprint
      ? {
          confidence: e.fingerprint.confidence,
          technologies: e.fingerprint.technologies,
          agentFrameworks: e.fingerprint.agentFrameworks,
          aiComponents: e.fingerprint.aiComponents,
          attackRecommendations: e.fingerprint.attackRecommendations,
          methodsUsed: e.fingerprint.methodsUsed,
          primaryProvider: e.fingerprint.primaryProvider,
          apiStyle: e.fingerprint.apiStyle,
        }
      : null,
  }));
}

export function mapReports(
  reports: ReportDto[],
  projects: ProjectDto[],
  scans: ScanDto[] = [],
): Report[] {
  const projectNames = new Map(projects.map((p) => [p.id, p.name]));
  const scanNames = new Map(scans.map((s) => [s.id, s.name]));
  return reports.map((r) => ({
    id: r.id,
    projectId: r.project_id,
    projectName: projectNames.get(r.project_id) ?? "",
    scanId: r.scan_id,
    scanName: r.scan_id ? scanNames.get(r.scan_id) ?? r.scan_id.slice(0, 8) : "—",
    title: r.name,
    format: coerceReportFormat(r.format),
    status: coerceJobStatus(r.status),
    findingCount: r.finding_count,
    createdAt: r.created_at,
    sizeBytes: 0,
  }));
}

function countBy<T>(items: T[], key: (item: T) => string): Map<string, number> {
  const map = new Map<string, number>();
  for (const item of items) {
    const k = key(item);
    map.set(k, (map.get(k) ?? 0) + 1);
  }
  return map;
}
