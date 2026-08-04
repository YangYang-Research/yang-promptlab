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

export function prettyJson(value: unknown): string {
  if (typeof value === "string") {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function stateLabel(status: string): string {
  const normalized = status.trim().toLowerCase();
  if (normalized === "ok" || normalized === "success") return "ok";
  if (normalized === "error" || normalized === "failed") return "error";
  if (normalized === "running") return "running";
  return status || "—";
}
