import { yazgChatThreadTitle } from "@/features/yazg/yazgChatSession";

export function formatTraceTime(ts?: string | null): string {
  if (!ts) return "—";
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
}

export function formatExecutionTime(ms?: number | null): string {
  if (ms == null || Number.isNaN(ms)) return "—";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 2 : 1)} s`;
  const mins = Math.floor(seconds / 60);
  const rem = seconds - mins * 60;
  return `${mins}m ${rem.toFixed(0)}s`;
}

export function formatTokenCount(tokens?: number | null): string {
  if (tokens == null || tokens <= 0) return "—";
  return tokens.toLocaleString();
}

export function shortId(id: string, keep = 8): string {
  if (id.length <= keep + 1) return id;
  return `${id.slice(0, keep)}…`;
}

export function conversationLabel(sessionId?: string | null): {
  id: string;
  title: string;
} {
  if (!sessionId) return { id: "—", title: "—" };
  const id = sessionId.startsWith("yazg-chat:")
    ? sessionId.slice("yazg-chat:".length)
    : sessionId;
  const title = yazgChatThreadTitle(sessionId)?.trim() || "Untitled chat";
  return { id, title };
}

/**
 * Recursively parse embedded JSON strings so nested payloads (e.g. the
 * capability classifier's `content` field, which is a stringified JSON blob)
 * render as fully-expanded, readable JSON instead of an escaped one-liner.
 */
function expandEmbeddedJson(value: unknown, depth = 0): unknown {
  if (depth > 6) return value;
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (
      (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
      (trimmed.startsWith("[") && trimmed.endsWith("]"))
    ) {
      try {
        return expandEmbeddedJson(JSON.parse(trimmed), depth + 1);
      } catch {
        return value;
      }
    }
    return value;
  }
  if (Array.isArray(value)) {
    return value.map((item) => expandEmbeddedJson(item, depth + 1));
  }
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(value)) {
      out[key] = expandEmbeddedJson(val, depth + 1);
    }
    return out;
  }
  return value;
}

export function prettyJson(value: unknown): string {
  const expanded = expandEmbeddedJson(value);
  try {
    return JSON.stringify(expanded, null, 2);
  } catch {
    return typeof expanded === "string" ? expanded : String(expanded);
  }
}

export function stateLabel(status: string): string {
  const normalized = status.trim().toLowerCase();
  if (normalized === "ok" || normalized === "success") return "ok";
  if (normalized === "error" || normalized === "failed") return "error";
  if (normalized === "running") return "running";
  return status || "—";
}
