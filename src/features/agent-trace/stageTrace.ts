export type StageEventLike = {
  agent: string;
  kind: string;
  message: string;
  timestamp?: string;
  conversationId?: string | null;
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

export type ConversationTrace = {
  id: string;
  conversationId: string;
  label: string;
  eventCount: number;
  pairCount: number;
  firstTimestamp?: string;
  lastTimestamp?: string;
  groups: StageTraceGroup[];
};

export const UNKNOWN_CONVERSATION_ID = "(no conversation id)";

/** Request stage → expected response stage. */
const RESPONSE_FOR: Record<string, string> = {
  completion_call: "completion_response",
  llm_request: "llm_response",
  llm_request_retry_text_only: "llm_response",
  tool_call_request: "tool_result_response",
  capability_classify_request: "capability_classify_response",
};

const REQUEST_STAGES = new Set(Object.keys(RESPONSE_FOR));
const RESPONSE_STAGES = new Set(Object.values(RESPONSE_FOR));

/** Stages shown as LLM wire request/response bodies. */
const LLM_WIRE_STAGES = new Set([
  "llm_request",
  "llm_response",
  "llm_request_retry_text_only",
]);

/** Stages worth showing in the Trace timeline (pairs + singles). */
const TRACE_TIMELINE_STAGES = new Set([
  ...LLM_WIRE_STAGES,
  "capability_classify_request",
  "capability_classify_response",
  "completion_call",
  "completion_response",
  "tool_call_request",
  "tool_result_response",
  "model_turn_finished",
]);

export function isLlmWireStage(stage: string | null | undefined): boolean {
  return stage != null && LLM_WIRE_STAGES.has(stage);
}

export function isTraceTimelineStage(stage: string | null | undefined): boolean {
  if (stage == null) return false;
  if (TRACE_TIMELINE_STAGES.has(stage)) return true;
  return (
    stage.startsWith("capability_") ||
    stage.startsWith("llm_") ||
    stage.startsWith("tool_") ||
    stage.startsWith("completion_")
  );
}

function familyForStage(stage: string | null): string {
  if (!stage) return "event";
  if (stage.startsWith("capability_")) return "Capability";
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

function conversationLabel(conversationId: string): string {
  if (conversationId === UNKNOWN_CONVERSATION_ID) return conversationId;
  if (conversationId.startsWith("yazg-chat:")) {
    return conversationId.slice("yazg-chat:".length) || conversationId;
  }
  return conversationId;
}

function eventTimestamp(group: StageTraceGroup): string | undefined {
  if (group.kind === "pair") {
    return group.request.event.timestamp ?? group.response.event.timestamp;
  }
  return group.item.event.timestamp;
}

function isBodyGroup(group: StageTraceGroup): boolean {
  if (group.kind === "pair") {
    return (
      isTraceTimelineStage(group.request.stage) ||
      isTraceTimelineStage(group.response.stage)
    );
  }
  return isTraceTimelineStage(group.item.stage);
}

/**
 * Bucket Yazg stage events by conversation id, newest conversations first.
 * Detail groups keep capability / LLM / tool / completion timeline bodies.
 */
export function groupByConversation(events: StageEventLike[]): ConversationTrace[] {
  const buckets = new Map<string, StageEventLike[]>();
  for (const event of events) {
    const id =
      (event.conversationId && event.conversationId.trim()) || UNKNOWN_CONVERSATION_ID;
    const list = buckets.get(id);
    if (list) list.push(event);
    else buckets.set(id, [event]);
  }

  const conversations: ConversationTrace[] = [];
  for (const [conversationId, bucket] of buckets) {
    const sorted = [...bucket].sort((a, b) =>
      (a.timestamp ?? "").localeCompare(b.timestamp ?? ""),
    );
    const allGroups = groupStageEvents(sorted);
    const groups = allGroups.filter(isBodyGroup);
    const timestamps = sorted
      .map((e) => e.timestamp)
      .filter((t): t is string => Boolean(t));
    conversations.push({
      id: conversationId,
      conversationId,
      label: conversationLabel(conversationId),
      eventCount: sorted.length,
      pairCount: groups.filter((g) => g.kind === "pair").length,
      firstTimestamp: timestamps[0],
      lastTimestamp: timestamps[timestamps.length - 1],
      groups,
    });
  }

  conversations.sort((a, b) =>
    (b.lastTimestamp ?? "").localeCompare(a.lastTimestamp ?? ""),
  );
  return conversations;
}

export function stageLabel(stage: string | null, fallbackKind: string): string {
  return stage ?? fallbackKind;
}

export function groupTimestamp(group: StageTraceGroup): string | undefined {
  return eventTimestamp(group);
}
