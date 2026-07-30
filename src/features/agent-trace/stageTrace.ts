export type StageEventLike = {
  agent: string;
  kind: string;
  message: string;
  timestamp?: string;
};

export type StageRole = "request" | "response" | "other";

export type ParsedStageEvent = {
  index: number;
  event: StageEventLike;
  stage: string | null;
  role: StageRole;
  body: unknown;
  pretty: string;
};

export type StageTraceGroup =
  | {
      kind: "pair";
      id: string;
      family: string;
      request: ParsedStageEvent;
      response: ParsedStageEvent;
    }
  | {
      kind: "single";
      id: string;
      item: ParsedStageEvent;
    };

/** Request stage → expected response stage. */
const RESPONSE_FOR: Record<string, string> = {
  completion_call: "completion_response",
  llm_request: "llm_response",
  llm_request_retry_text_only: "llm_response",
  tool_call_request: "tool_result_response",
};

const REQUEST_STAGES = new Set(Object.keys(RESPONSE_FOR));
const RESPONSE_STAGES = new Set(Object.values(RESPONSE_FOR));

function familyForStage(stage: string | null): string {
  if (!stage) return "event";
  if (stage.startsWith("llm_")) return "LLM";
  if (stage.startsWith("tool_")) return "Tool";
  if (stage.startsWith("completion_")) return "Completion";
  if (stage.startsWith("model_turn")) return "Turn";
  if (stage.startsWith("hook_")) return "Hook";
  if (stage.startsWith("salvage") || stage.startsWith("direct_") || stage.startsWith("reply_")) {
    return "Reply";
  }
  return stage;
}

function roleForStage(stage: string | null): StageRole {
  if (!stage) return "other";
  if (REQUEST_STAGES.has(stage) || stage.endsWith("_request") || stage.endsWith("_call")) {
    return "request";
  }
  if (
    RESPONSE_STAGES.has(stage) ||
    stage.endsWith("_response") ||
    stage.endsWith("_result") ||
    stage.endsWith("_finished")
  ) {
    return "response";
  }
  return "other";
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

export function parseStageEvent(event: StageEventLike, index: number): ParsedStageEvent {
  let stage: string | null = null;
  let body: unknown = event.message;
  let pretty = event.message;

  try {
    const parsed = JSON.parse(event.message) as { stage?: unknown; body?: unknown };
    if (parsed && typeof parsed === "object") {
      if (typeof parsed.stage === "string") {
        stage = parsed.stage;
      }
      if ("body" in parsed) {
        body = parsed.body;
        pretty = prettyJson(parsed.body);
      } else {
        pretty = prettyJson(parsed);
      }
    }
  } catch {
    const match = /^stage=([^\s]+)/.exec(event.message);
    if (match) stage = match[1] ?? null;
  }

  return {
    index,
    event,
    stage,
    role: roleForStage(stage),
    body,
    pretty,
  };
}

/**
 * Group sequential stage events into request/response pairs when stages match.
 * Unmatched events stay as singles (preserves timeline order).
 */
export function groupStageEvents(events: StageEventLike[]): StageTraceGroup[] {
  const parsed = events.map((event, index) => parseStageEvent(event, index));
  const groups: StageTraceGroup[] = [];
  const used = new Set<number>();

  for (let i = 0; i < parsed.length; i += 1) {
    if (used.has(i)) continue;
    const item = parsed[i]!;
    const expected = item.stage != null ? (RESPONSE_FOR[item.stage] ?? null) : null;

    if (expected) {
      let responseIdx = -1;
      for (let j = i + 1; j < parsed.length; j += 1) {
        if (used.has(j)) continue;
        const candidate = parsed[j]!;
        if (candidate.stage === expected) {
          responseIdx = j;
          break;
        }
        if (candidate.stage != null && RESPONSE_FOR[candidate.stage] === expected) {
          break;
        }
      }
      if (responseIdx >= 0) {
        used.add(i);
        used.add(responseIdx);
        groups.push({
          kind: "pair",
          id: `pair-${i}-${responseIdx}`,
          family: familyForStage(item.stage),
          request: item,
          response: parsed[responseIdx]!,
        });
        continue;
      }
    }

    used.add(i);
    groups.push({
      kind: "single",
      id: `single-${i}`,
      item,
    });
  }

  return groups;
}

export function stageLabel(stage: string | null, fallbackKind: string): string {
  return stage ?? fallbackKind;
}
