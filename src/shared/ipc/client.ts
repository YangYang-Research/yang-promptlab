import { invokeCommand } from "./invoke";

export type HealthResponse = {
  status: string;
  version: string;
};

export type AppInfoResponse = {
  name: string;
  version: string;
  identifier: string;
};

export function healthCheck(): Promise<HealthResponse> {
  return invokeCommand<HealthResponse>("health");
}

export function getAppInfo(): Promise<AppInfoResponse> {
  return invokeCommand<AppInfoResponse>("app_info");
}

// ---------------------------------------------------------------------------
// Domain DTOs (mirror the Rust command responses; timestamps are RFC 3339)
// ---------------------------------------------------------------------------

export type { ProjectDto } from "./projects";
export {
  createProject,
  deleteProject,
  getProject,
  listProjects,
} from "./projects";

export type TargetDto = {
  id: string;
  project_id: string;
  name: string;
  target_type: string;
  descriptor: unknown;
  profile: unknown;
  created_at: string;
  updated_at: string;
};

export type ScanDto = {
  id: string;
  project_id: string;
  target_id: string | null;
  name: string;
  status: string;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

export type ScanDetailDto = ScanDto & {
  playbook: unknown | null;
};

export type FindingDto = {
  id: string;
  scan_id: string;
  project_id: string;
  target_id: string | null;
  title: string;
  severity: string;
  category: string | null;
  description: string | null;
  evidence: unknown;
  status: string;
  created_at: string;
  updated_at: string;
};

export type EndpointDto = {
  id: string;
  scan_id: string;
  target_id: string | null;
  url: string;
  kind: string;
  method: string | null;
  confidence: number;
  evidence: string | null;
  source_url: string | null;
  discovered_at: string;
  endpoint_type: string;
  ai_framework: string | null;
  risk_score: number;
  metadata_confidence: number;
  discovery_source: string;
  auth_required: boolean;
  metadata: AiEndpointMetadataDto | null;
  attack_recommendations: EndpointAttackRecommendationDto[];
};

export type AiEndpointMetadataDto = {
  basic: EndpointBasicDto;
  fingerprint: FingerprintMetadataDto;
  schema: SchemaMetadataDto;
  inference: InferenceFieldsDto;
  capabilities: EndpointCapabilitiesDto;
  classification: EndpointClassificationDto;
  risk: RiskAssessmentDto;
  provenance: DiscoveryProvenanceDto;
  raw?: RawObservationDto | null;
};

export type EndpointBasicDto = {
  id: string;
  url: string;
  method: string;
  host: string;
  protocol: string;
  status: number;
};

export type FingerprintMetadataDto = {
  framework: string;
  provider: string;
  version: string;
  confidence: number;
  apiStyle: string;
  technologies: string[];
};

export type SchemaMetadataDto = {
  contentType: string | null;
  requestSchema: NormalizedSchemaDto | null;
  responseSchema: NormalizedSchemaDto | null;
  transport: string[];
};

export type NormalizedSchemaDto = {
  format: string;
  fields: SchemaFieldDto[];
};

export type SchemaFieldDto = {
  name: string;
  fieldType: string;
  required: boolean;
};

export type InferenceFieldsDto = {
  promptField: string | null;
  historyField: string | null;
  conversationField: string | null;
  modelField: string | null;
  streamField: string | null;
  toolField: string | null;
  attachmentField: string | null;
};

export type EndpointCapabilitiesDto = {
  supportsChat: boolean;
  supportsStreaming: boolean;
  supportsEmbedding: boolean;
  supportsVision: boolean;
  supportsTools: boolean;
  supportsJsonMode: boolean;
  supportsThinking: boolean;
  supportsMemory: boolean;
  supportsAgent: boolean;
};

export type EndpointClassificationDto = {
  endpointType: string;
  aiFramework: string;
  confidence: number;
  riskScore: number;
};

export type RiskAssessmentDto = {
  score: number;
  factors: string[];
};

export type DiscoveryProvenanceDto = {
  discoverySource: string;
  authenticationRequired: boolean;
  discoveredAt: string;
  kind: string;
  evidence: string | null;
};

export type RawObservationDto = {
  requestHeaders: Record<string, string>;
  requestBody: string | null;
  responseHeaders: Record<string, string>;
  responseBody: string | null;
};

export type EndpointAttackRecommendationDto = {
  category: string;
  reason: string;
  priority: number;
};

/** @deprecated Use metadata + attack_recommendations */
export type EndpointFingerprintDto = {
  confidence: number;
  technologies: FingerprintTechnologyDto[];
  agentFrameworks: FingerprintFrameworkDto[];
  aiComponents: FingerprintComponentDto[];
  attackRecommendations: FingerprintRecommendationDto[];
  methodsUsed: string[];
  primaryProvider: string | null;
  apiStyle: string | null;
  platformProfile: PlatformProfileDto;
};

export type PlatformProfileDto = {
  platform: string;
  version: string;
  authType: string;
  llmProvider: string;
  memoryEnabled: boolean;
  toolsEnabled: boolean;
  ragEnabled: boolean;
};

export type FingerprintTechnologyDto = {
  id: string;
  name: string;
  category: string;
  confidence: number;
  signals: string[];
};

export type FingerprintFrameworkDto = {
  id: string;
  name: string;
  confidence: number;
  signals: string[];
};

export type FingerprintComponentDto = {
  id: string;
  name: string;
  confidence: number;
  signals: string[];
};

export type FingerprintRecommendationDto = {
  category: string;
  reason: string;
  priority: number;
};

export type AttackRunDto = {
  scan: ScanDto;
  category: string;
  attempts: number;
  successes: number;
  findings: FindingDto[];
};

export type ScanStartDto = {
  scan_id: string;
};

export type ScanStartRequest = {
  projectId: string;
  targetId: string;
  profile: string;
  categories: string[];
  disabledTests?: string[];
  generatorMode?: string;
  payloadStrategy?: import("@/features/scans/payloadStrategy").PayloadStrategyDto;
  agentMode?: boolean;
  maxAgentAttempts?: number;
  draftScanId?: string;
};

export type ScanStatusDto = {
  scan_id: string;
  status: string;
  progress_percent: number;
  completed: number;
  total: number;
  findings_count: number;
  current_endpoint: string | null;
  current_test: string | null;
  started_at: string | null;
  agent_mode: boolean;
  current_phase: string | null;
  current_attempt: number | null;
  current_retry: number | null;
};

export type ScanProgressEvent = {
  scanId: string;
  timestamp: string;
  level: "INFO" | "WARN" | "ERROR";
  message: string;
  endpoint?: string;
  payload?: string;
  statusCode?: number;
  latency?: number;
  findingId?: string;
};

export type ReportDto = {
  id: string;
  project_id: string;
  scan_id: string | null;
  name: string;
  format: string;
  status: string;
  file_path: string | null;
  finding_count: number;
  created_at: string;
  updated_at: string;
};

export type ReportContentDto = {
  id: string;
  name: string;
  format: string;
  content: string;
};

// ---------------------------------------------------------------------------
// Domain command wrappers (Tauri auto-maps snake_case Rust args -> camelCase JS)
// ---------------------------------------------------------------------------

export const listTargets = (projectId: string) =>
  invokeCommand<TargetDto[]>("target_list", { projectId });

export const getTarget = (id: string) => invokeCommand<TargetDto>("target_get", { id });

export const getTargetWizardDescriptor = (id: string) =>
  invokeCommand<TargetDto>("target_wizard_descriptor", { id });

export const updateTargetDescriptor = (id: string, descriptor: unknown) =>
  invokeCommand<TargetDto>("target_update_descriptor", { id, descriptor });

export const createTarget = (
  projectId: string,
  name: string,
  targetType: string,
  descriptor?: unknown,
) => invokeCommand<TargetDto>("target_create", { projectId, name, targetType, descriptor });

export const listScans = (projectId: string) =>
  invokeCommand<ScanDto[]>("scan_list", { projectId });

export const getScan = (id: string) => invokeCommand<ScanDetailDto>("scan_get", { id });

export const createScan = (
  projectId: string,
  name: string,
  targetId?: string | null,
  status?: string | null,
) =>
  invokeCommand<ScanDto>("scan_create", {
    projectId,
    targetId: targetId ?? null,
    name,
    status: status ?? null,
  });

export const listFindings = (scanId: string) =>
  invokeCommand<FindingDto[]>("finding_list", { scanId });

export const listFindingsAll = () =>
  invokeCommand<FindingDto[]>("finding_list_all");

export const generateReport = (
  projectId: string,
  scanId: string,
  format?: string,
  kind?: string,
) =>
  invokeCommand<ReportDto>("report_generate", { projectId, scanId, format, kind });

export const listReports = (projectId: string) =>
  invokeCommand<ReportDto[]>("report_list", { projectId });

export const listReportsAll = () =>
  invokeCommand<ReportDto[]>("report_list_all");

export const readReport = (id: string) =>
  invokeCommand<ReportContentDto>("report_read", { id });

export const exportReport = (id: string) =>
  invokeCommand<string>("report_export", { id });

export const listEndpoints = (scanId: string) =>
  invokeCommand<EndpointDto[]>("endpoint_list", { scanId });

export const createEndpoint = (
  scanId: string,
  targetId: string,
  method: string,
  path: string,
) =>
  invokeCommand<EndpointDto>("endpoint_create", { scanId, targetId, method, path });

export const updateEndpoint = (endpointId: string, method: string) =>
  invokeCommand<EndpointDto>("endpoint_update", { endpointId, method });

export const runPromptInjection = (endpointId: string) =>
  invokeCommand<AttackRunDto>("attack_run_prompt_injection", { endpointId });

export const startScan = (request: ScanStartRequest) =>
  invokeCommand<ScanStartDto>("scan_start", {
    projectId: request.projectId,
    targetId: request.targetId,
    profile: request.profile,
    categories: request.categories,
    disabledTests: request.disabledTests ?? [],
    generatorMode: request.generatorMode,
    payloadStrategy: request.payloadStrategy,
    agentMode: request.agentMode ?? false,
    maxAgentAttempts: request.maxAgentAttempts ?? 5,
    draftScanId: request.draftScanId,
  });

export const getScanStatus = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_status", { scanId });

export const pauseScan = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_pause", { scanId });

export const resumeScan = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_resume", { scanId });

export const stopScan = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_stop", { scanId });
