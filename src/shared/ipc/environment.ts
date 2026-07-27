import { invokeCommand } from "./invoke";

export type EnvironmentStatusDto = {
  root: string;
  config: string;
  workspaces: string;
  models: string;
  runtime: string;
  logs: string;
  plugins: string;
  cache: string;
  temp: string;
  backups: string;
  database: string;
  writable: boolean;
  message: string;
};

export type EnvironmentUpdateRequest = {
  root?: string | null;
  workspaces?: string | null;
  models?: string | null;
  runtime?: string | null;
  logs?: string | null;
  plugins?: string | null;
  cache?: string | null;
  temp?: string | null;
  backups?: string | null;
};

export type LogFileInfoDto = {
  name: string;
  path: string;
};

export type LogsTailResponse = {
  path: string;
  content: string;
};

export type OcsfEventDto = {
  timestamp: string;
  severity: string;
  category: string;
  classUid: number;
  className: string;
  activityId: number;
  activityName: string;
  module: string;
  component: string;
  workspaceId?: string | null;
  projectId?: string | null;
  scanId?: string | null;
  message: string;
  attributes: Record<string, unknown>;
};

export type DbHealthDto = {
  connected: boolean;
  path: string;
  sizeBytes: number;
};

export const getEnvironment = () =>
  invokeCommand<EnvironmentStatusDto>("environment_get");

export const getDbHealth = () => invokeCommand<DbHealthDto>("db_health");

export const openRootDirectory = () => invokeCommand<void>("environment_open_root");

export const updateEnvironment = (request: EnvironmentUpdateRequest) =>
  invokeCommand<EnvironmentStatusDto>("environment_update", { request });

export const listLogFiles = () =>
  invokeCommand<LogFileInfoDto[]>("logs_list_files");

export const tailLogFile = (fileName: string, maxBytes?: number) =>
  invokeCommand<LogsTailResponse>("logs_tail", { fileName, maxBytes: maxBytes ?? null });

export const getRecentLogEvents = (limit?: number) =>
  invokeCommand<OcsfEventDto[]>("logs_recent_events", { limit: limit ?? null });

export const openLogsFolder = () => invokeCommand<void>("logs_open_folder");

export type LiveLogCategory =
  | "application"
  | "system"
  | "runtime"
  | "models"
  | "authentication"
  | "harness"
  | "planner"
  | "payload_generator"
  | "attack_engine"
  | "judge"
  | "workspace"
  | "projects"
  | "plugins"
  | "settings"
  | "user_interface"
  | "scan";

export type LiveLogSeverity =
  | "informational"
  | "low"
  | "medium"
  | "high"
  | "critical";

export type EmitLiveLogRequest = {
  category: LiveLogCategory;
  severity?: LiveLogSeverity;
  activityName: string;
  message: string;
  module?: string;
  component?: string;
  projectId?: string | null;
  scanId?: string | null;
  attributes?: Record<string, unknown>;
};

/** Fire-and-forget publish into Settings → Troubleshooting live logs. */
export function emitLiveLog(request: EmitLiveLogRequest): void {
  void invokeCommand<void>("logs_emit", {
    request: {
      category: request.category,
      severity: request.severity ?? null,
      activityName: request.activityName,
      message: request.message,
      module: request.module ?? null,
      component: request.component ?? null,
      projectId: request.projectId ?? null,
      scanId: request.scanId ?? null,
      attributes: request.attributes ?? {},
    },
  }).catch(() => {
    // Live logs must never block wizard UX (mock mode / IPC unavailable).
  });
}
