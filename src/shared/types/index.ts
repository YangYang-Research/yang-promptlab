export type Severity = "critical" | "high" | "medium" | "low" | "info";

export type JobStatus = "pending" | "draft" | "running" | "paused" | "completed" | "failed" | "cancelled";

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
  createdAt: string;
  lastScanAt: string | null;
  fingerprint: string | null;
  tags: string[];
  authType: string;
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
  verdict: "vulnerable" | "not_vulnerable" | null;
  discoveredAt: string;
  evidence: unknown;
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
  endpointType?: string;
  aiFramework?: string | null;
  riskScore?: number;
  metadataConfidence?: number;
  discoverySource?: string;
  authRequired?: boolean;
  metadata?: import("@/shared/ipc/client").AiEndpointMetadataDto | null;
  attackRecommendations?: import("@/shared/ipc/client").EndpointAttackRecommendationDto[];
  /** @deprecated use metadata */
  fingerprint: EndpointFingerprint | null;
};

export type EndpointFingerprint = {
  confidence: number;
  technologies: FingerprintTechnology[];
  agentFrameworks: FingerprintFramework[];
  aiComponents: FingerprintComponent[];
  attackRecommendations: FingerprintRecommendation[];
  methodsUsed: string[];
  primaryProvider: string | null;
  apiStyle: string | null;
  platformProfile: PlatformProfile;
};

export type PlatformProfile = {
  platform: string;
  version: string;
  authType: string;
  llmProvider: string;
  memoryEnabled: boolean;
  toolsEnabled: boolean;
  ragEnabled: boolean;
};

export type FingerprintTechnology = {
  id: string;
  name: string;
  category: string;
  confidence: number;
  signals: string[];
};

export type FingerprintFramework = {
  id: string;
  name: string;
  confidence: number;
  signals: string[];
};

export type FingerprintComponent = {
  id: string;
  name: string;
  confidence: number;
  signals: string[];
};

export type FingerprintRecommendation = {
  category: string;
  reason: string;
  priority: number;
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
  type: "attack" | "finding" | "report" | "target";
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
