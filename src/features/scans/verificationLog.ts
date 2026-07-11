import { isLikelyAuthHeader } from "./targetDescriptor";
import type { VerificationConsoleEntryDto } from "./targetProfile";

export const VERIFICATION_LOG_START_AUTH = "Start verify authentication";
export const VERIFICATION_LOG_START_AI = "Start Analyze Endpoint";

export type VerificationLogResponseKind = "connectivity" | "ai_probe" | "ai_validation" | "error";

export type VerificationLogLine = {
  id: string;
  timestamp: string;
  message: string;
};

export function appendVerificationLogLine(
  log: VerificationLogLine[],
  message: string,
): VerificationLogLine[] {
  return [
    ...log,
    {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      timestamp: new Date().toISOString(),
      message,
    },
  ];
}

export function appendVerificationLogLines(
  log: VerificationLogLine[],
  messages: string[],
): VerificationLogLine[] {
  return messages.reduce((next, message) => appendVerificationLogLine(next, message), log);
}

export function formatVerificationLogTime(timestamp: string): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return timestamp;
  return date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function maskSensitiveCredential(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length <= 8) return "***";
  if (trimmed.length <= 24) {
    return `${trimmed.slice(0, 4)}***${trimmed.slice(-2)}`;
  }
  return `${trimmed.slice(0, 16)}***${trimmed.slice(-20)}`;
}

export function maskHeaderCredentialValue(value: string): string {
  const bearer = value.match(/^(Bearer\s+)(.+)$/i);
  if (bearer) {
    return `Bearer ${maskSensitiveCredential(bearer[2]!)}`;
  }
  const basic = value.match(/^(Basic\s+)(.+)$/i);
  if (basic) {
    return `Basic ${maskSensitiveCredential(basic[2]!)}`;
  }
  const token = value.match(/^(Token\s+)(.+)$/i);
  if (token) {
    return `Token ${maskSensitiveCredential(token[2]!)}`;
  }
  return maskSensitiveCredential(value);
}

export function formatAuthenticationLogLines(headers: Record<string, string>): string[] {
  const authLines = Object.entries(headers)
    .filter(([name]) => isLikelyAuthHeader(name))
    .map(
      ([name, value]) =>
        `Step 1 — Auth header applied: ${name} = ${maskHeaderCredentialValue(value)}`,
    );

  if (authLines.length === 0) {
    return ["Step 1 — Auth header applied: none"];
  }
  return authLines;
}

export function formatSendRequestLogLine(
  requestLog: string,
  step: 1 | 2 = 1,
): string {
  const prefix = `Step ${step} — Outbound probe request`;
  const trimmed = requestLog.trim();
  if (!trimmed) return `${prefix}: (empty)`;
  return `${prefix}:\n${trimmed}`;
}

function responseLogPrefix(kind: VerificationLogResponseKind): string {
  switch (kind) {
    case "ai_validation":
      return "Step 2 — AI validation result";
    case "ai_probe":
      return "Step 2 — Capability probe result";
    case "error":
      return "Verification failed";
    default:
      return "Step 1 — Connectivity result";
  }
}

export function formatResponseLogLine(
  console: VerificationConsoleEntryDto,
  kind: VerificationLogResponseKind = "connectivity",
): string {
  const prefix = responseLogPrefix(kind);

  if (kind === "ai_validation") {
    const message = console.message?.trim();
    if (message) {
      return `${prefix}: ${message}`;
    }
    return `${prefix}: (no Yazg classification message)`;
  }

  const meta: string[] = [];
  if (console.statusCode > 0) {
    meta.push(`HTTP ${console.statusCode}`);
  }
  if (console.responseTimeMs > 0) {
    meta.push(`${console.responseTimeMs}ms`);
  }

  const preview = console.responsePreview?.trim();
  if (preview) {
    const header =
      meta.length > 0 ? `${prefix}: ${meta.join(" · ")}` : `${prefix}:`;
    return `${header}\n${preview}`;
  }

  if (console.message?.trim()) {
    const suffix = meta.length > 0 ? ` · ${meta.join(" · ")}` : "";
    return `${prefix}: ${console.message.trim()}${suffix}`;
  }

  return meta.length > 0 ? `${prefix}: ${meta.join(" · ")}` : `${prefix}:`;
}

export function formatAiValidationLogLine(message: string): string {
  const trimmed = message.trim();
  if (!trimmed) {
    return "Step 2 — AI validation result: (no Yazg classification message)";
  }
  return `Step 2 — AI validation result: ${trimmed}`;
}

export function formatErrorLogLine(message: string): string {
  return `Verification failed: ${message.trim()}`;
}

export function migrateLegacyVerificationConsole(
  legacyConsole: VerificationConsoleEntryDto,
): VerificationLogLine[] {
  const lines = [
    VERIFICATION_LOG_START_AUTH,
    ...formatAuthenticationLogLines(legacyConsole.headers ?? {}),
  ];
  if (legacyConsole.url) {
    lines.push(`Step 1 — Outbound probe request:\ncurl --location '${legacyConsole.url}'`);
  }
  lines.push(formatResponseLogLine(legacyConsole, "connectivity"));
  return lines.reduce<VerificationLogLine[]>(
    (log, message) => appendVerificationLogLine(log, message),
    [],
  );
}
