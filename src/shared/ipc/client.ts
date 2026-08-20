import { invokeCommand } from "./invoke";

export type HealthResponse = {
  status: string;
  version: string;
};

export type AppInfoResponse = {
  name: string;
  version: string;
  identifier: string;
  platform: string;
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
  retries?: Array<{ at: string; mode: string }>;
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

export type FindingImportDto = {
  scan_id: string;
  imported_count: number;
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
  reflectionEnabled?: boolean;
  adaptivePlanning?: boolean;
  draftScanId?: string;
  /** Re-run only categories listed in scan progress `categories_failed`. */
  retryFailedOnly?: boolean;
  /** Retry Scan: resume from last incomplete stage. Start Attack is a fresh run. */
  continueFromProgress?: boolean;
};

export type ScanStatusDto = {
  scan_id: string;
  status: string;
  progress_percent: number;
  completed: number;
  total: number;
  categories_completed?: number;
  categories_failed?: string[];
  categories_succeeded?: string[];
  attacks_completed?: number;
  attacks_total?: number;
  testcases_completed?: number;
  testcases_total?: number;
  pause_pending?: boolean;
  findings_count: number;
  current_endpoint: string | null;
  current_test: string | null;
  started_at: string | null;
  agent_mode: boolean;
  current_phase: string | null;
  current_attempt: number | null;
  current_retry: number | null;
  /** Live phase trail — e.g. generate → attack → recover → attack. */
  phase_trail?: string[];
};

export type ScanProgressEvent = {
  scanId: string;
  timestamp: string;
  level: "INFO" | "WARN" | "ERROR";
  message: string;
  endpoint?: string;
  payload?: string;
  response?: string;
  statusCode?: number;
  latency?: number;
  findingId?: string;
};

export type ScanConsoleTailDto = {
  content: string;
  offset: number;
  totalBytes: number;
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

export const deleteTarget = (id: string) => invokeCommand<void>("target_delete", { id });

export const listScans = (projectId: string) =>
  invokeCommand<ScanDto[]>("scan_list", { projectId });

export const getScan = (id: string) => invokeCommand<ScanDetailDto>("scan_get", { id });

export const listFindings = (scanId: string) =>
  invokeCommand<FindingDto[]>("finding_list", { scanId });

export const listFindingsAll = () =>
  invokeCommand<FindingDto[]>("finding_list_all");

export const importFindingsSarif = (path: string, projectId?: string | null) =>
  invokeCommand<FindingImportDto>("finding_import_sarif", {
    path,
    projectId: projectId || null,
  });

export const updateFindingStatus = (id: string, status: string) =>
  invokeCommand<FindingDto>("finding_update", { id, status });

export const rejudgeFinding = (id: string) =>
  invokeCommand<FindingDto>("finding_rejudge", { id });

export const deleteFinding = (id: string) =>
  invokeCommand<void>("finding_delete", { id });

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

export const exportScanReport = (
  projectId: string,
  scanId: string,
  format?: string,
  kind?: string,
) =>
  invokeCommand<string>("report_export_scan", { projectId, scanId, format, kind });

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
    reflectionEnabled: request.reflectionEnabled ?? false,
    adaptivePlanning: request.adaptivePlanning ?? false,
    draftScanId: request.draftScanId,
    retryFailedOnly: request.retryFailedOnly ?? false,
    continueFromProgress: request.continueFromProgress ?? false,
  });

export const getScanStatus = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_status", { scanId });

export const pauseScan = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_pause", { scanId });

export const resumeScan = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_resume", { scanId });

export const stopScan = (scanId: string) =>
  invokeCommand<ScanStatusDto>("scan_stop", { scanId });

export const tailScanConsole = (scanId: string, offset?: number) =>
  invokeCommand<ScanConsoleTailDto>("scan_console_tail", {
    scanId,
    offset: offset ?? null,
  });

export const deleteScan = (scanId: string) =>
  invokeCommand<null>("scan_delete", { id: scanId });
