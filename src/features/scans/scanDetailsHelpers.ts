type AuthDescriptor = {
  kind?: string;
  engine?: string;
  method?: string;
  session_id?: string | null;
  config?: Record<string, unknown>;
  username?: string;
  header?: string;
  header_name?: string;
  value?: string;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

import type { ScanDetailDto } from "@/shared/ipc";
import type { JobStatus, ScanRun } from "@/shared/types";

export function mapScanDetailToRun(dto: ScanDetailDto): ScanRun {
  return {
    id: dto.id,
    projectId: dto.project_id,
    targetId: dto.target_id,
    name: dto.name,
    status: dto.status as JobStatus,
    startedAt: dto.started_at,
    completedAt: dto.completed_at,
    createdAt: dto.created_at,
  };
}

export function extractTargetUrl(descriptor: unknown): string {
  const obj = asRecord(descriptor);
  if (obj && typeof obj.url === "string") return obj.url;
  if (obj && typeof obj.base_url === "string") return obj.base_url;
  return "";
}

export function extractAuthKind(
  descriptor: unknown,
): "none" | "username_password" | "sso" | "basic" | "api_key" | "jwt" {
  const obj = asRecord(descriptor);
  const auth = asRecord(obj?.auth) as AuthDescriptor | null;
  const kind = auth?.kind ?? "none";
  switch (kind) {
    case "username_password":
    case "sso":
    case "basic":
    case "api_key":
    case "jwt":
      return kind;
    default:
      return "none";
  }
}

export function extractAuthType(descriptor: unknown): string {
  const obj = asRecord(descriptor);
  const auth = asRecord(obj?.auth) as AuthDescriptor | null;
  const kind = auth?.kind ?? "none";

  switch (kind) {
    case "username_password":
      return "Username / Password";
    case "sso":
      return "SSO";
    case "basic":
      return "Basic";
    case "api_key":
      return "API Key";
    case "jwt":
      return "JWT";
    default:
      return "None";
  }
}

export function extractAuthSummary(descriptor: unknown): string {
  const obj = asRecord(descriptor);
  const auth = asRecord(obj?.auth) as AuthDescriptor | null;
  if (!auth?.kind || auth.kind === "none") return "No authentication configured";

  const config = asRecord(auth.config);

  switch (auth.kind) {
    case "username_password":
      return [
        config?.login_url ? `Login: ${config.login_url}` : null,
        config?.username ? `User: ${config.username}` : null,
        auth.session_id ? `Session: ${auth.session_id}` : "Session not recorded yet",
      ]
        .filter(Boolean)
        .join(" · ");
    case "sso":
      return [
        config?.login_url ? `Login: ${config.login_url}` : null,
        config?.success_url_pattern ? `Success: ${config.success_url_pattern}` : null,
        auth.session_id ? `Session: ${auth.session_id}` : "Session not recorded yet",
      ]
        .filter(Boolean)
        .join(" · ");
    case "basic":
      return config?.username
        ? `HTTP Basic user: ${config.username}`
        : auth.username
          ? `HTTP Basic user: ${auth.username}`
          : "HTTP Basic authentication";
    case "api_key":
      return config?.header_name
        ? `Header: ${config.header_name}`
        : auth.header
          ? `Header: ${auth.header}`
          : "API key header configured";
    case "jwt":
      return config?.header_name
        ? `JWT via ${config.header_name}`
        : "Configured JWT bearer token";
    default:
      return auth.kind;
  }
}

export function isManualEndpoint(kind: string, sourceUrl: string | null): boolean {
  return kind === "manual" || sourceUrl === "manual";
}

export function formatDurationMs(ms: number | null | undefined): string {
  if (ms == null || ms <= 0) return "—";
  if (ms < 1000) return `${ms} ms`;
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rem = seconds % 60;
  return `${minutes}m ${rem}s`;
}

export function formatTimestamp(value: string | null | undefined): string {
  if (!value) return "—";
  return new Date(value).toLocaleString();
}
