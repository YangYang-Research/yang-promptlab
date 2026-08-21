export type Severity = "critical" | "high" | "medium" | "low" | "info";

export type JobStatus = "pending" | "draft" | "running" | "paused" | "completed" | "failed" | "cancelled";

/** Lifecycle status for a target (derived from verification + scan history). */
export type TargetStatus = "pending" | "verified" | "scanned";

export type ProjectStatus = "active" | "archived" | "draft";

export type TargetType = "web" | "api" | "llm" | "mobile";

export type ReportFormat = "html" | "pdf" | "json" | "sarif" | "markdown" | "csv";

export type ModelStatus = "installed" | "downloading" | "available" | "error";

export type AttackCategory =
  | "prompt_injection"
  | "jailbreak"
  | "data_exfiltration"
  | "model_dos"
  | "supply_chain"
  | "insecure_output"
  | "excessive_agency"
  | "system_prompt_leak"
  | "rag_poisoning";

export type Project = {
  id: string;
  name: string;
  description: string;
  status: ProjectStatus;
  createdAt: string;
  updatedAt: string;
  targetCount: number;
  findingCount: number;
  /** Persisted health score 0–100; `null` = N/A (not scored yet). */
  healthScore: number | null;
  owner: string;
};

export type Target = {
  id: string;
  projectId: string;
  name: string;
  url: string;
  type: TargetType;
  /** Human label from saved target profile provider (e.g. OpenRouter). */
  providerLabel: string | null;
  status: TargetStatus;
  createdAt: string;
  lastScanAt: string | null;
  fingerprint: string | null;
  tags: string[];
  authType: string;
  authKind: "none" | "username_password" | "sso" | "basic" | "api_key" | "jwt";
};

export type AttackRun = {
  id: string;
  targetId: string;
  targetName: string;
  category: AttackCategory;
  status: JobStatus;
  payloadsTotal: number;
  payloadsRun: number;
  findingsCount: number;
  startedAt: string;
  completedAt: string | null;
};

export type Finding = {
  id: string;
  scanId: string;
  projectId: string;
  targetId: string;
  targetName: string;
  /** Full endpoint URL from the linked target, when available. */
  targetUrl: string;
  title: string;
  description: string;
  severity: Severity;
  category: string;
  status: "open" | "confirmed" | "false_positive" | "fixed";
  confidence: number;
  verdict: "vulnerable" | "not_vulnerable" | null;
  discoveredAt: string;
  evidence: unknown;
};

export type ScanRetry = {
  at: string;
  mode: string;
};

export type ScanRun = {
  id: string;
  projectId: string;
  targetId: string | null;
  name: string;
  status: JobStatus;
  startedAt: string | null;
  completedAt: string | null;
  createdAt: string;
  retries?: ScanRetry[];
};

export type Report = {
  id: string;
  projectId: string;
  projectName: string;
  scanId: string | null;
  scanName: string;
  title: string;
  format: ReportFormat;
  status: JobStatus;
  findingCount: number;
  createdAt: string;
  sizeBytes: number;
  /** User export to Downloads; false/undefined = in-app generated report. */
  exported?: boolean;
};

export type LocalModel = {
  id: string;
  name: string;
  provider: string;
  sizeGb: number;
  status: ModelStatus;
  downloadProgress: number;
  quant: string;
  path: string | null;
  sha256: string | null;
};

export type ActivityItem = {
  id: string;
  type: "attack" | "finding" | "report" | "target" | "runtime" | "model";
  message: string;
  timestamp: string;
  severity?: Severity;
};

export type DashboardStats = {
  projects: number;
  activeProjects: number;
  targets: number;
  scanningTargets: number;
  openFindings: number;
  criticalFindings: number;
  runningScans: number;
  installedModels: number;
  downloadingModels: number;
};
