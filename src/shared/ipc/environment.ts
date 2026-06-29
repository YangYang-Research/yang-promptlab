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

export const getEnvironment = () =>
  invokeCommand<EnvironmentStatusDto>("environment_get");

export const openRootDirectory = () => invokeCommand<void>("environment_open_root");

export const updateEnvironment = (request: EnvironmentUpdateRequest) =>
  invokeCommand<EnvironmentStatusDto>("environment_update", { request });

export const listLogFiles = () =>
  invokeCommand<LogFileInfoDto[]>("logs_list_files");

export const tailLogFile = (fileName: string, maxBytes?: number) =>
  invokeCommand<LogsTailResponse>("logs_tail", { fileName, maxBytes: maxBytes ?? null });

export const getRecentLogEvents = (limit?: number) =>
  invokeCommand<OcsfEventDto[]>("logs_recent_events", { limit: limit ?? null });

export const getLogsFolderPath = () =>
  invokeCommand<string>("logs_open_folder");
