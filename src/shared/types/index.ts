export type Severity = "critical" | "high" | "medium" | "low" | "info";

export type JobStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export type ProjectStatus = "active" | "archived" | "draft";

export type TargetType = "web" | "api" | "llm" | "mobile";

export type ReportFormat = "html" | "pdf" | "json" | "sarif" | "markdown";

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
  owner: string;
};

export type Target = {
  id: string;
  projectId: string;
  name: string;
  url: string;
  type: TargetType;
  status: JobStatus;
  lastScanAt: string | null;
  fingerprint: string | null;
  tags: string[];
};

export type DiscoveryJob = {
  id: string;
  targetId: string;
  targetName: string;
  status: JobStatus;
  progress: number;
  endpointsFound: number;
  startedAt: string;
  completedAt: string | null;
  modules: string[];
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
  title: string;
  description: string;
  severity: Severity;
  category: string;
  status: "open" | "confirmed" | "false_positive" | "fixed";
  confidence: number;
  discoveredAt: string;
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
};

export type DiscoveredEndpoint = {
  id: string;
  scanId: string;
  targetId: string | null;
  url: string;
  kind: string;
  method: string | null;
  confidence: number;
  evidence: string | null;
  sourceUrl: string | null;
  discoveredAt: string;
};

export type Report = {
  id: string;
  projectId: string;
  projectName: string;
  title: string;
  format: ReportFormat;
  status: JobStatus;
  findingCount: number;
  createdAt: string;
  sizeBytes: number;
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
  type: "discovery" | "attack" | "finding" | "report";
  message: string;
  timestamp: string;
  severity?: Severity;
};

export type DashboardStats = {
  projects: number;
  targets: number;
  openFindings: number;
  criticalFindings: number;
  runningScans: number;
  installedModels: number;
};
