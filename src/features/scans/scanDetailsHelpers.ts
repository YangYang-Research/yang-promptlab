type AuthDescriptor = {
  kind?: string;
  username?: string;
  header?: string;
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

export function extractAuthType(descriptor: unknown): string {
  const obj = asRecord(descriptor);
  const auth = asRecord(obj?.auth) as AuthDescriptor | null;
  const kind = auth?.kind ?? "none";
  switch (kind) {
    case "basic":
      return "Username / password";
    case "api_key":
      return "API key";
    case "sso":
      return "SSO";
    default:
      return "None";
  }
}

export function extractAuthSummary(descriptor: unknown): string {
  const obj = asRecord(descriptor);
  const auth = asRecord(obj?.auth) as AuthDescriptor | null;
  if (!auth?.kind || auth.kind === "none") return "No authentication configured";

  switch (auth.kind) {
    case "basic":
      return auth.username ? `Basic auth user: ${auth.username}` : "Basic authentication";
    case "api_key":
      return auth.header ? `Header: ${auth.header}` : "API key header configured";
    case "sso":
      return "Browser SSO session (Playwright integration pending)";
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
