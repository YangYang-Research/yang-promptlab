import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Card,
  EmptyState,
  IconButton,
  IconCheck,
  IconCopy,
  IconTimeline,
  IconTree,
  PageHeader,
  PageLoadingSkeleton,
  RefreshButton,
  StatusBadge,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  getAgentTraceDetail,
  type AgentTraceSpanDto,
} from "@/shared/ipc/agentTrace";
import { useToast } from "@/shared/notifications";

import {
  conversationLabel,
  formatExecutionTime,
  formatTokenCount,
  formatTraceTime,
  prettyJson,
  stateLabel,
} from "./traceFormat";

function DetailRow({
  label,
  value,
}: {
  label: string;
  value: ReactNode;
}) {
  return (
    <div className="detail-row">
      <span className="detail-row__label">{label}</span>
      <span className="detail-row__value">{value}</span>
    </div>
  );
}

function SummaryStat({
  label,
  value,
}: {
  label: string;
  value: ReactNode;
}) {
  return (
    <div className="summary-stat">
      <span className="summary-stat__label">{label}</span>
      <span className="summary-stat__value summary-stat__value--text">{value}</span>
    </div>
  );
}

function PayloadBody({
  text,
  defaultOpen = false,
  collapsible = true,
}: {
  text: string;
  defaultOpen?: boolean;
  collapsible?: boolean;
}) {
  const long =
    collapsible && (text.length > 480 || text.split("\n").length > 12);
  if (!long) {
    return <pre className="agent-trace-payload__body">{text}</pre>;
  }
  return (
    <details className="agent-trace-payload__fold" open={defaultOpen}>
      <summary>Payload · {text.length.toLocaleString()} chars</summary>
      <pre className="agent-trace-payload__body">{text}</pre>
    </details>
  );
}

function CopyPayloadButton({
  text,
  label,
}: {
  text: string;
  label: string;
}) {
  const { notify } = useToast();
  const [copied, setCopied] = useState(false);
  const value = text.trim();
  if (!value || value === "—") return null;

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      notify(`${label} copied`, "success");
      window.setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      notify(
        error instanceof Error ? error.message : `Failed to copy ${label}`,
        "error",
      );
    }
  }

  return (
    <IconButton
      ariaLabel={copied ? `${label} copied` : `Copy ${label}`}
      size="sm"
      active={copied}
      onClick={() => void handleCopy()}
    >
      {copied ? <IconCheck /> : <IconCopy />}
    </IconButton>
  );
}

function kindClass(kind: string): string {
  const k = kind.toLowerCase();
  if (k.includes("capability")) return "capability";
  if (k === "llm") return "llm";
  if (k.includes("tool")) return "tool";
  if (k === "agent") return "agent";
  return "other";
}

type SpansTab = "summary" | "details";

type ChatMessage = {
  role: string;
  content: string;
};

type TraceSummary = {
  input: string;
  turn: number | null;
  output: string;
  capability: string | null;
  confidence: number | null;
  reason: string | null;
  wireMessages: ChatMessage[];
  wireFunctionName: string;
  model: string | null;
  outputRole: string;
  outputContent: string;
  executionTimeMs: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return null;
}

function messageContent(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed || null;
  }
  if (Array.isArray(value)) {
    const parts = value
      .map((part) => {
        if (typeof part === "string") return part;
        const rec = asRecord(part);
        if (!rec) return null;
        if (typeof rec.text === "string") return rec.text;
        if (typeof rec.content === "string") return rec.content;
        return null;
      })
      .filter((part): part is string => Boolean(part?.trim()));
    const joined = parts.join("\n").trim();
    return joined || null;
  }
  return null;
}

function lastUserMessage(inputs: unknown): string | null {
  const rec = asRecord(inputs);
  const messages = rec?.messages;
  if (!Array.isArray(messages)) return null;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = asRecord(messages[i]);
    if (!msg || msg.role !== "user") continue;
    const content = messageContent(msg.content);
    if (content) return content;
  }
  return null;
}

function userTurnCount(inputs: unknown): number | null {
  const rec = asRecord(inputs);
  const messages = rec?.messages;
  if (!Array.isArray(messages)) return null;
  let count = 0;
  for (const entry of messages) {
    const msg = asRecord(entry);
    if (msg?.role === "user") count += 1;
  }
  return count > 0 ? count : null;
}

function roleLabel(role: string): string {
  const r = role.trim().toLowerCase();
  if (r === "system") return "System";
  if (r === "user") return "User";
  if (r === "assistant") return "Assistant";
  if (r === "tool") return "Tool";
  if (!r) return "Message";
  return role.charAt(0).toUpperCase() + role.slice(1);
}

function parseWireMessages(inputs: unknown): ChatMessage[] {
  const rec = asRecord(inputs);
  const messages = rec?.messages;
  if (!Array.isArray(messages)) return [];
  const out: ChatMessage[] = [];
  for (const entry of messages) {
    const msg = asRecord(entry);
    if (!msg) continue;
    const role = typeof msg.role === "string" ? msg.role : "message";
    const content =
      messageContent(msg.content) ??
      (typeof msg.name === "string" ? `(tool: ${msg.name})` : null);
    if (!content) continue;
    out.push({ role, content });
  }
  return out;
}

function extractModel(inputs: unknown, attributes?: Record<string, string>): string | null {
  const rec = asRecord(inputs);
  if (typeof rec?.model === "string" && rec.model.trim()) return rec.model.trim();
  if (typeof attributes?.model === "string" && attributes.model.trim()) {
    return attributes.model.trim();
  }
  const params = asRecord(rec?.model_params);
  if (typeof params?.model === "string" && params.model.trim()) {
    return params.model.trim();
  }
  return null;
}

function buildTraceSummary(
  spans: AgentTraceSpanDto[],
  traceLatencyMs?: number | null,
  traceTags?: Record<string, string>,
): TraceSummary {
  const capabilitySpan =
    spans.find((s) => kindClass(s.kind) === "capability") ?? null;
  const llmSpans = spans.filter((s) => kindClass(s.kind) === "llm");

  const capIn = asRecord(capabilitySpan?.inputs);
  const capOut = asRecord(capabilitySpan?.outputs);

  const input =
    (typeof capIn?.latest_user_message === "string"
      ? capIn.latest_user_message.trim()
      : "") ||
    llmSpans.map((s) => lastUserMessage(s.inputs)).find(Boolean) ||
    "—";

  const turn =
    llmSpans.map((s) => userTurnCount(s.inputs)).find((n) => n != null) ?? null;

  let output = "—";
  let toolFallback: string | null = null;
  for (let i = llmSpans.length - 1; i >= 0; i -= 1) {
    const out = asRecord(llmSpans[i]?.outputs);
    const content = messageContent(out?.content);
    if (content) {
      output = content;
      break;
    }
    if (toolFallback) continue;
    const toolCalls = out?.tool_calls;
    if (Array.isArray(toolCalls) && toolCalls.length > 0) {
      const names = toolCalls
        .map((call) => {
          const rec = asRecord(call);
          return (
            (typeof rec?.name === "string" && rec.name) ||
            (typeof asRecord(rec?.function)?.name === "string" &&
              (asRecord(rec?.function)?.name as string)) ||
            null
          );
        })
        .filter((name): name is string => Boolean(name));
      if (names.length > 0) {
        toolFallback = `Tool calls: ${names.join(", ")}`;
      }
    }
  }
  if (output === "—" && toolFallback) output = toolFallback;

  const capability =
    (typeof capOut?.capability === "string" && capOut.capability) ||
    (typeof capabilitySpan?.attributes?.capability === "string" &&
      capabilitySpan.attributes.capability) ||
    null;
  const confidence =
    typeof capOut?.confidence === "number"
      ? capOut.confidence
      : typeof capabilitySpan?.metrics?.confidence === "number"
        ? capabilitySpan.metrics.confidence
        : null;
  const reason =
    typeof capOut?.reason === "string" && capOut.reason.trim()
      ? capOut.reason.trim()
      : null;

  const wireSpan =
    llmSpans.find((s) => parseWireMessages(s.inputs).length > 0) ??
    llmSpans[0] ??
    null;
  const wireMessages = wireSpan ? parseWireMessages(wireSpan.inputs) : [];
  const wireFunctionName =
    wireSpan?.name?.trim() ||
    (wireSpan ? kindClass(wireSpan.kind) : "") ||
    "completion";
  const model =
    (wireSpan
      ? extractModel(wireSpan.inputs, wireSpan.attributes)
      : null) ||
    (typeof traceTags?.model === "string" && traceTags.model.trim()
      ? traceTags.model.trim()
      : null) ||
    (capabilitySpan
      ? extractModel(capabilitySpan.inputs, capabilitySpan.attributes)
      : null);
  // Prefer whole-trace wall time (same as Trace Metrics). Fall back to LLM span.
  const executionTimeMs =
    traceLatencyMs ??
    wireSpan?.latencyMs ??
    (typeof wireSpan?.metrics?.latency_ms === "number"
      ? wireSpan.metrics.latency_ms
      : null);

  const tokenMetric = (key: string): number | null => {
    const value = wireSpan?.metrics?.[key];
    return typeof value === "number" && Number.isFinite(value)
      ? Math.max(0, Math.round(value))
      : null;
  };
  const inputTokens = tokenMetric("input_tokens");
  const outputTokens = tokenMetric("output_tokens");

  let outputContent = output;
  let outputRole = "assistant";
  for (let i = llmSpans.length - 1; i >= 0; i -= 1) {
    const out = asRecord(llmSpans[i]?.outputs);
    if (!out) continue;
    const content = messageContent(out.content);
    if (!content) continue;
    outputContent = content;
    if (typeof out.role === "string" && out.role.trim()) {
      outputRole = out.role.trim();
    }
    break;
  }

  return {
    input,
    turn,
    output,
    capability,
    confidence,
    reason,
    wireMessages,
    wireFunctionName,
    model,
    outputRole,
    outputContent,
    executionTimeMs,
    inputTokens,
    outputTokens,
  };
}

function SummaryFold({
  title,
  hint,
  defaultOpen = true,
  children,
}: {
  title: string;
  hint?: string;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  return (
    <details className="agent-trace-summary-fold" open={defaultOpen}>
      <summary className="agent-trace-summary-fold__summary">
        <span className="agent-trace-summary-fold__title">{title}</span>
        {hint ? (
          <span className="agent-trace-summary-fold__hint">{hint}</span>
        ) : null}
      </summary>
      <div className="agent-trace-summary-fold__body">{children}</div>
    </details>
  );
}

function SummaryField({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="agent-trace-summary-field">
      <span className="agent-trace-summary-field__label">{label}</span>
      <div className="agent-trace-value-card">{children}</div>
    </div>
  );
}

function TraceSummaryView({
  spans,
  traceLatencyMs,
  traceTags,
}: {
  spans: AgentTraceSpanDto[];
  traceLatencyMs?: number | null;
  traceTags?: Record<string, string>;
}) {
  const summary = useMemo(
    () => buildTraceSummary(spans, traceLatencyMs, traceTags),
    [spans, traceLatencyMs, traceTags],
  );
  const capabilityHint = summary.capability
    ? summary.confidence != null
      ? `${summary.capability} · ${(summary.confidence * 100).toFixed(0)}%`
      : summary.capability
    : undefined;
  const inputHint =
    summary.inputTokens != null
      ? `${formatTokenCount(summary.inputTokens)} tokens`
      : "User message";
  const outputHint =
    summary.outputTokens != null
      ? `${formatTokenCount(summary.outputTokens)} tokens`
      : "Assistant reply";

  return (
    <div className="agent-trace-summary-folds">
      <SummaryFold title="Input" hint={inputHint}>
        <div className="agent-trace-summary-fold__stack">
          <SummaryField label="Message">{summary.input}</SummaryField>
          <SummaryField label="Turn">
            {summary.turn != null ? String(summary.turn) : "—"}
          </SummaryField>
          <details className="agent-trace-io-fold">
            <summary className="agent-trace-io-fold__summary">
              <span className="agent-trace-io-fold__fn">
                <span className="agent-trace-io-fold__fn-name mono">
                  {summary.wireFunctionName}
                </span>
                <span className="agent-trace-io-fold__fn-suffix">was called</span>
              </span>
              <span className="agent-trace-io-fold__hint">
                {formatExecutionTime(summary.executionTimeMs)}
              </span>
            </summary>
            <div className="agent-trace-io-fold__body">
              <section className="agent-trace-io-section">
                <h3 className="agent-trace-io-section__title">Inputs</h3>
                <div className="agent-trace-summary-field">
                  <span className="agent-trace-summary-field__label">
                    messages
                  </span>
                  {summary.wireMessages.length > 0 ? (
                    <div className="agent-trace-io-messages">
                      {summary.wireMessages.map((msg, idx) => (
                        <div
                          key={`${msg.role}-${idx}`}
                          className="agent-trace-value-card agent-trace-value-card--message"
                        >
                          <span className="agent-trace-io-message__role">
                            {roleLabel(msg.role)}
                          </span>
                          <span className="agent-trace-io-message__content">
                            {msg.content}
                          </span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="agent-trace-value-card">—</div>
                  )}
                </div>
                <SummaryField label="model">
                  {summary.model ?? "—"}
                </SummaryField>
              </section>
              <section className="agent-trace-io-section">
                <h3 className="agent-trace-io-section__title">Outputs</h3>
                <SummaryField label="role">{summary.outputRole}</SummaryField>
                <SummaryField label="content">
                  {summary.outputContent}
                </SummaryField>
              </section>
            </div>
          </details>
        </div>
      </SummaryFold>
      <SummaryFold title="Output" hint={outputHint}>
        <div className="agent-trace-summary-fold__stack">
          <SummaryField label="Response">{summary.output}</SummaryField>
        </div>
      </SummaryFold>
      <SummaryFold title="Capability" hint={capabilityHint}>
        {summary.capability || summary.reason || summary.confidence != null ? (
          <div className="agent-trace-summary-fold__stack">
            <SummaryField label="Capability">
              {summary.capability ?? "—"}
            </SummaryField>
            <SummaryField label="Confidence">
              {summary.confidence != null
                ? `${(summary.confidence * 100).toFixed(0)}%`
                : "—"}
            </SummaryField>
            <SummaryField label="Reason">
              {summary.reason ?? "—"}
            </SummaryField>
          </div>
        ) : (
          <p className="text-muted text-sm">
            No capability classification on this trace.
          </p>
        )}
      </SummaryFold>
    </div>
  );
}

function SpanCard({
  span,
  spanRef,
}: {
  span: AgentTraceSpanDto;
  spanRef?: (node: HTMLElement | null) => void;
}) {
  const family = kindClass(span.kind);
  const duration = spanEffectiveDurationMs(span);
  const inputText = span.inputs != null ? prettyJson(span.inputs) : "—";
  const outputText = span.outputs != null ? prettyJson(span.outputs) : "—";
  return (
    <details
      ref={spanRef}
      id={`span-${span.id}`}
      className={`agent-trace-group agent-trace-group--${family}`}
      open
    >
      <summary className="agent-trace-group__head">
        <span
          className={`agent-trace-group__family agent-trace-group__family--${family}`}
        >
          {span.kind}
        </span>
        <span className="agent-trace-group__stages">
          <span className="agent-trace-io-fold__fn-name mono">{span.name}</span>
          <span className="agent-trace-io-fold__fn-suffix"> was called</span>
        </span>
        <span className="agent-trace-group__time">
          {duration != null ? `${formatBarDuration(duration)} · ` : ""}
          {formatTraceTime(span.startedAt)}
        </span>
      </summary>
      <div className="agent-trace-group__body">
        <div className="agent-trace-group__grid">
          <section className="agent-trace-payload agent-trace-payload--req">
            <header className="agent-trace-payload__head">
              <span className="agent-trace-payload__badge">In</span>
              <span className="agent-trace-payload__stage">Inputs</span>
              <span className="agent-trace-payload__actions">
                <CopyPayloadButton text={inputText} label="Inputs" />
              </span>
            </header>
            <PayloadBody text={inputText} collapsible={false} />
          </section>
          <section className="agent-trace-payload agent-trace-payload--res">
            <header className="agent-trace-payload__head">
              <span className="agent-trace-payload__badge">Out</span>
              <span className="agent-trace-payload__stage">Outputs</span>
              <span className="agent-trace-payload__meta">{span.status}</span>
              <span className="agent-trace-payload__actions">
                <CopyPayloadButton text={outputText} label="Outputs" />
              </span>
            </header>
            <PayloadBody text={outputText} defaultOpen />
          </section>
        </div>
      </div>
    </details>
  );
}

type BreakdownNode = {
  id: string;
  name: string;
  kind: string;
  /** null = synthetic root (yazg_turn / yazg). */
  span: AgentTraceSpanDto | null;
  children: BreakdownNode[];
  /** Trace-level timing for synthetic `yazg_turn` root. */
  traceMeta?: {
    startedAt: string;
    endedAt?: string | null;
    latencyMs?: number | null;
  };
};

type TraceBreakdownMeta = {
  id: string;
  name: string;
  startedAt: string;
  endedAt?: string | null;
  latencyMs?: number | null;
};

/**
 * Infer parent links when backend did not persist them (older traces: LLM span
 * closes before the tool hook runs, so tool spans were stored as roots).
 */
function resolveSpanParentIds(spans: AgentTraceSpanDto[]): Map<string, string | null> {
  const byId = new Map(spans.map((s) => [s.id, s]));
  const parentById = new Map<string, string | null>();

  for (const span of spans) {
    const explicit = span.parentSpanId?.trim() || "";
    parentById.set(
      span.id,
      explicit && byId.has(explicit) ? explicit : null,
    );
  }

  const sorted = [...spans].sort((a, b) =>
    a.startedAt.localeCompare(b.startedAt),
  );
  let lastLlmId: string | null = null;
  for (const span of sorted) {
    const family = kindClass(span.kind);
    if (family === "llm") {
      lastLlmId = span.id;
      continue;
    }
    if (family === "tool" && lastLlmId && parentById.get(span.id) == null) {
      parentById.set(span.id, lastLlmId);
    }
  }

  return parentById;
}

/**
 * Forest for one Yazg turn:
 *   yazg_turn (trace wall-clock)
 *     └── yazg (agent entry → nested span calls)
 *           ├── capability_classify
 *           └── completion / tools…
 */
function buildBreakdownForest(
  spans: AgentTraceSpanDto[],
  trace: TraceBreakdownMeta,
): BreakdownNode[] {
  const byId = new Map(spans.map((s) => [s.id, s]));
  const parentById = resolveSpanParentIds(spans);
  const children = new Map<string, AgentTraceSpanDto[]>();

  for (const span of spans) {
    const parent = parentById.get(span.id)?.trim() || "";
    if (!parent || !byId.has(parent)) continue;
    const list = children.get(parent) ?? [];
    list.push(span);
    children.set(parent, list);
  }

  function nodeFromSpan(span: AgentTraceSpanDto): BreakdownNode {
    return {
      id: span.id,
      name: span.name,
      kind: span.kind,
      span,
      children: (children.get(span.id) ?? []).map(nodeFromSpan),
    };
  }

  const roots = spans
    .filter((s) => {
      const parent = parentById.get(s.id)?.trim() || "";
      return !parent || !byId.has(parent);
    })
    .map(nodeFromSpan);

  if (roots.length === 0) return [];

  const yazgAgent: BreakdownNode = {
    id: "__yazg_agent__",
    name: "yazg",
    kind: "agent",
    span: null,
    children: roots,
  };

  return [
    {
      id: "__yazg_turn__",
      name: trace.name?.trim() || "yazg_turn",
      kind: "agent",
      span: null,
      traceMeta: {
        startedAt: trace.startedAt,
        endedAt: trace.endedAt,
        latencyMs: trace.latencyMs,
      },
      children: [yazgAgent],
    },
  ];
}

function countDescendants(node: BreakdownNode): number {
  let n = 0;
  for (const child of node.children) {
    n += 1 + countDescendants(child);
  }
  return n;
}

function BreakdownTreeRow({
  node,
  depth,
  selectedId,
  expanded,
  onToggle,
  onSelect,
  isLast,
}: {
  node: BreakdownNode;
  depth: number;
  selectedId: string | null;
  expanded: Set<string>;
  onToggle: (id: string) => void;
  onSelect: (id: string) => void;
  isLast: boolean;
}) {
  const hasChildren = node.children.length > 0;
  const isOpen = expanded.has(node.id);
  const family = kindClass(node.kind);
  const selected = node.span != null && selectedId === node.id;
  const childCount = countDescendants(node);
  const isRoot = depth === 0;

  return (
    <li
      className={[
        "agent-trace-tree__node",
        depth > 0 ? "agent-trace-tree__node--branch" : "",
        isLast ? "agent-trace-tree__node--last" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div
        className={[
          "agent-trace-tree__row",
          `agent-trace-tree__row--${family}`,
          selected ? "agent-trace-tree__row--selected" : "",
          isRoot ? "agent-trace-tree__row--root" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {hasChildren ? (
          <button
            type="button"
            className={`agent-trace-tree__toggle${
              isOpen ? " agent-trace-tree__toggle--open" : ""
            }`}
            aria-expanded={isOpen}
            aria-label={isOpen ? "Collapse" : "Expand"}
            onClick={(e) => {
              e.stopPropagation();
              onToggle(node.id);
            }}
          />
        ) : (
          <span className="agent-trace-tree__toggle-spacer" aria-hidden />
        )}
        <button
          type="button"
          className="agent-trace-tree__label"
          aria-current={selected ? "true" : undefined}
          onClick={() => {
            if (node.span) onSelect(node.id);
            else if (hasChildren) onToggle(node.id);
          }}
        >
          <span className={`agent-trace-tree__dot agent-trace-tree__dot--${family}`} />
          <span
            className={`agent-trace-tree__name mono${
              isRoot ? " agent-trace-tree__name--root" : ""
            }`}
          >
            {node.name}
          </span>
          {hasChildren ? (
            <span className="agent-trace-tree__count">{childCount}</span>
          ) : null}
        </button>
      </div>
      {hasChildren && isOpen ? (
        <ul className="agent-trace-tree__children">
          {node.children.map((child, index) => (
            <BreakdownTreeRow
              key={child.id}
              node={child}
              depth={depth + 1}
              selectedId={selectedId}
              expanded={expanded}
              onToggle={onToggle}
              onSelect={onSelect}
              isLast={index === node.children.length - 1}
            />
          ))}
        </ul>
      ) : null}
    </li>
  );
}

function collectExpandableIds(nodes: BreakdownNode[], into: Set<string>) {
  for (const node of nodes) {
    if (node.children.length > 0) {
      into.add(node.id);
      collectExpandableIds(node.children, into);
    }
  }
}

function TraceBreakdownTree({
  spans,
  trace,
  selectedId,
  onSelect,
}: {
  spans: AgentTraceSpanDto[];
  trace: TraceBreakdownMeta;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const forest = useMemo(
    () => buildBreakdownForest(spans, trace),
    [spans, trace],
  );

  const [expanded, setExpanded] = useState<Set<string>>(() => {
    const initial = new Set<string>();
    collectExpandableIds(forest, initial);
    return initial;
  });

  useEffect(() => {
    const next = new Set<string>();
    collectExpandableIds(forest, next);
    setExpanded(next);
  }, [forest]);

  function toggle(id: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  if (forest.length === 0) {
    return <p className="text-muted text-sm">No function spans on this turn.</p>;
  }

  return (
    <ul className="agent-trace-tree">
      {forest.map((node, index) => (
        <BreakdownTreeRow
          key={node.id}
          node={node}
          depth={0}
          selectedId={selectedId}
          expanded={expanded}
          onToggle={toggle}
          onSelect={onSelect}
          isLast={index === forest.length - 1}
        />
      ))}
    </ul>
  );
}

type BreakdownViewMode = "span-tree" | "execution-timeline";

function BreakdownViewToggle({
  viewMode,
  onChange,
}: {
  viewMode: BreakdownViewMode;
  onChange: (mode: BreakdownViewMode) => void;
}) {
  return (
    <div className="view-mode-toggle" role="group" aria-label="Breakdown view">
      <IconButton
        ariaLabel="Span Tree"
        size="sm"
        active={viewMode === "span-tree"}
        onClick={() => onChange("span-tree")}
      >
        <IconTree />
      </IconButton>
      <IconButton
        ariaLabel="Execution Timeline"
        size="sm"
        active={viewMode === "execution-timeline"}
        onClick={() => onChange("execution-timeline")}
      >
        <IconTimeline />
      </IconButton>
    </div>
  );
}

type GanttTiming = {
  /** Offset from trace t0 — bar start. */
  startMs: number;
  /** Offset from trace t0 — bar end (wall-clock envelope). */
  endMs: number;
  /** Duration shown in the label (own span, or sum of children for synthetic). */
  durationMs: number;
};

function spanMetricLatencyMs(span: AgentTraceSpanDto): number | null {
  const metric = span.metrics?.latency_ms;
  if (typeof metric === "number" && Number.isFinite(metric) && metric > 0) {
    return metric;
  }
  return null;
}

/** Effective span duration — same source for Gantt + SpanCard. */
function spanEffectiveDurationMs(span: AgentTraceSpanDto): number | null {
  const metric = spanMetricLatencyMs(span);
  if (metric != null) return metric;
  if (span.latencyMs != null && Number.isFinite(span.latencyMs)) {
    return Math.max(0, span.latencyMs);
  }
  if (span.endedAt) {
    const start = Date.parse(span.startedAt);
    const end = Date.parse(span.endedAt);
    if (Number.isFinite(start) && Number.isFinite(end) && end >= start) {
      return end - start;
    }
  }
  return null;
}

function spanAbsoluteBounds(span: AgentTraceSpanDto): {
  start: number;
  end: number;
} {
  const start = Date.parse(span.startedAt);
  const startSafe = Number.isFinite(start) ? start : 0;
  const ended = span.endedAt ? Date.parse(span.endedAt) : NaN;
  const endSafe =
    Number.isFinite(ended) && ended >= startSafe ? ended : startSafe;

  const duration = spanEffectiveDurationMs(span);
  if (duration != null && duration > 0) {
    const wall = Math.max(0, endSafe - startSafe);
    // Older traces opened capability after the LLM finished — backdate start.
    if (wall < duration * 0.5) {
      return { start: endSafe - duration, end: endSafe };
    }
    return { start: startSafe, end: startSafe + duration };
  }
  return { start: startSafe, end: endSafe };
}

function nodeTiming(node: BreakdownNode, t0: number): GanttTiming {
  if (node.traceMeta) {
    const start = Date.parse(node.traceMeta.startedAt);
    const startSafe = Number.isFinite(start) ? start : t0;
    let endSafe = startSafe;
    if (node.traceMeta.endedAt) {
      const end = Date.parse(node.traceMeta.endedAt);
      if (Number.isFinite(end) && end >= startSafe) endSafe = end;
    }
    const latency =
      node.traceMeta.latencyMs != null &&
      Number.isFinite(node.traceMeta.latencyMs)
        ? Math.max(0, node.traceMeta.latencyMs)
        : Math.max(0, endSafe - startSafe);
    if (latency > 0 && endSafe <= startSafe) {
      endSafe = startSafe + latency;
    }
    const startMs = Math.max(0, startSafe - t0);
    const endMs = Math.max(startMs, endSafe - t0);
    return { startMs, endMs, durationMs: latency > 0 ? latency : endMs - startMs };
  }
  if (node.span) {
    const { start, end } = spanAbsoluteBounds(node.span);
    const startMs = Math.max(0, start - t0);
    const endMs = Math.max(startMs, end - t0);
    return { startMs, endMs, durationMs: endMs - startMs };
  }
  if (node.children.length === 0) {
    return { startMs: 0, endMs: 0, durationMs: 0 };
  }
  let startMs = Infinity;
  let endMs = 0;
  let durationMs = 0;
  for (const child of node.children) {
    const t = nodeTiming(child, t0);
    startMs = Math.min(startMs, t.startMs);
    endMs = Math.max(endMs, t.endMs);
    // Synthetic parents (e.g. yazg): label = sum of child durations.
    durationMs += t.durationMs;
  }
  if (!Number.isFinite(startMs)) startMs = 0;
  return { startMs, endMs, durationMs };
}

function formatAxisSeconds(sec: number): string {
  if (sec === 0) return "0s";
  if (sec < 1) return `${sec.toFixed(2)}s`;
  if (sec < 10) return `${sec.toFixed(2)}s`;
  if (sec < 60) return `${sec.toFixed(1)}s`;
  const m = Math.floor(sec / 60);
  const r = sec - m * 60;
  return r > 0.05 ? `${m}m ${r.toFixed(0)}s` : `${m}m`;
}

function formatBarDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const sec = ms / 1000;
  if (sec < 10) return `${sec.toFixed(2)}s`;
  if (sec < 60) return `${sec.toFixed(1)}s`;
  return formatExecutionTime(ms);
}

function niceAxisTicks(totalMs: number): { ticks: number[]; axisMs: number } {
  const totalSec = Math.max(totalMs / 1000, 0.001);
  const candidates = [
    0.05, 0.1, 0.2, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600,
  ];
  const rough = totalSec / 4;
  const step = candidates.find((n) => n >= rough) ?? Math.ceil(rough);
  const ticks: number[] = [];
  for (let t = 0; t <= totalSec + step * 0.001; t += step) {
    ticks.push(Number(t.toFixed(6)));
  }
  const last = ticks[ticks.length - 1] ?? totalSec;
  const axisMs = Math.max(totalMs, last * 1000);
  return { ticks, axisMs };
}

type GanttRowModel = {
  node: BreakdownNode;
  depth: number;
  timing: GanttTiming;
};

function flattenGanttRows(
  nodes: BreakdownNode[],
  expanded: Set<string>,
  t0: number,
  depth: number,
  into: GanttRowModel[],
) {
  for (const node of nodes) {
    into.push({ node, depth, timing: nodeTiming(node, t0) });
    if (node.children.length > 0 && expanded.has(node.id)) {
      flattenGanttRows(node.children, expanded, t0, depth + 1, into);
    }
  }
}

function ExecutionTimelineGantt({
  spans,
  trace,
  selectedId,
  onSelect,
}: {
  spans: AgentTraceSpanDto[];
  trace: TraceBreakdownMeta;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const forest = useMemo(
    () => buildBreakdownForest(spans, trace),
    [spans, trace],
  );
  const [expanded, setExpanded] = useState<Set<string>>(() => {
    const initial = new Set<string>();
    collectExpandableIds(forest, initial);
    return initial;
  });

  useEffect(() => {
    const next = new Set<string>();
    collectExpandableIds(forest, next);
    setExpanded(next);
  }, [forest]);

  const t0 = useMemo(() => {
    let min = Infinity;
    const traceStart = Date.parse(trace.startedAt);
    if (Number.isFinite(traceStart)) min = Math.min(min, traceStart);
    for (const span of spans) {
      const { start } = spanAbsoluteBounds(span);
      if (start < min) min = start;
    }
    return Number.isFinite(min) ? min : Date.now();
  }, [spans, trace.startedAt]);

  const rows = useMemo(() => {
    const into: GanttRowModel[] = [];
    flattenGanttRows(forest, expanded, t0, 0, into);
    return into;
  }, [forest, expanded, t0]);

  const rawTotalMs = useMemo(
    () => rows.reduce((max, row) => Math.max(max, row.timing.endMs), 0),
    [rows],
  );
  const { ticks, axisMs } = useMemo(
    () => niceAxisTicks(rawTotalMs),
    [rawTotalMs],
  );

  function toggle(id: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  if (forest.length === 0) {
    return <p className="text-muted text-sm">No function spans on this turn.</p>;
  }

  return (
    <div className="agent-trace-gantt" role="img" aria-label="Execution timeline">
      <div className="agent-trace-gantt__axis" aria-hidden>
        <span className="agent-trace-gantt__gutter" />
        <div className="agent-trace-gantt__axis-track">
          {ticks.map((sec) => (
            <span
              key={sec}
              className="agent-trace-gantt__tick"
              style={{ left: `${(sec * 1000) / axisMs * 100}%` }}
            >
              {formatAxisSeconds(sec)}
            </span>
          ))}
        </div>
        <span className="agent-trace-gantt__dur-col" />
      </div>
      <div className="agent-trace-gantt__rows">
        {rows.map(({ node, depth, timing }) => {
          const hasChildren = node.children.length > 0;
          const isOpen = expanded.has(node.id);
          const family = kindClass(node.kind);
          const selected =
            (node.span != null && selectedId === node.id) ||
            (node.span == null && selectedId === node.id);
          const leftPct = axisMs > 0 ? (timing.startMs / axisMs) * 100 : 0;
          const wallMs = Math.max(0, timing.endMs - timing.startMs);
          const widthPct =
            axisMs > 0 ? Math.max((wallMs / axisMs) * 100, 1.25) : 1.25;

          return (
            <div
              key={node.id}
              className={[
                "agent-trace-gantt__row",
                selected ? "agent-trace-gantt__row--selected" : "",
                `agent-trace-gantt__row--${family}`,
              ]
                .filter(Boolean)
                .join(" ")}
              style={{ ["--gantt-depth" as string]: String(depth) }}
            >
              <div className="agent-trace-gantt__gutter">
                {hasChildren ? (
                  <button
                    type="button"
                    className={`agent-trace-tree__toggle${
                      isOpen ? " agent-trace-tree__toggle--open" : ""
                    }`}
                    aria-expanded={isOpen}
                    aria-label={isOpen ? "Collapse" : "Expand"}
                    onClick={() => toggle(node.id)}
                  />
                ) : (
                  <span className="agent-trace-tree__toggle-spacer" aria-hidden />
                )}
              </div>
              <button
                type="button"
                className="agent-trace-gantt__track"
                onClick={() => onSelect(node.id)}
              >
                <span
                  className={`agent-trace-gantt__bar agent-trace-gantt__bar--${family}`}
                  style={{
                    left: `calc(${leftPct}% + ${depth * 0.55}rem)`,
                    width: `min(${widthPct}%, calc(100% - ${leftPct}% - ${depth * 0.55}rem))`,
                  }}
                  title={`${node.name} · ${formatBarDuration(timing.durationMs)}`}
                >
                  <span
                    className={`agent-trace-gantt__bar-dot agent-trace-tree__dot--${family}`}
                  />
                  <span className="agent-trace-gantt__bar-name mono">
                    {node.name}
                  </span>
                </span>
              </button>
              <span className="agent-trace-gantt__dur mono">
                {formatBarDuration(timing.durationMs)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function TraceDetailsView({
  spans,
  trace,
}: {
  spans: AgentTraceSpanDto[];
  trace: TraceBreakdownMeta;
}) {
  const [viewMode, setViewMode] = useState<BreakdownViewMode>("span-tree");
  const [selectedId, setSelectedId] = useState<string | null>(
    spans[0]?.id ?? null,
  );
  const nodeRefs = useRef(new Map<string, HTMLElement>());
  const traceMeta = useMemo<TraceBreakdownMeta>(
    () => ({
      id: trace.id,
      name: trace.name,
      startedAt: trace.startedAt,
      endedAt: trace.endedAt,
      latencyMs: trace.latencyMs,
    }),
    [trace.id, trace.name, trace.startedAt, trace.endedAt, trace.latencyMs],
  );

  useEffect(() => {
    if (!spans.some((s) => s.id === selectedId)) {
      setSelectedId(spans[0]?.id ?? null);
    }
  }, [spans, selectedId]);

  function selectSpan(id: string) {
    setSelectedId(id);
    const node = nodeRefs.current.get(id);
    node?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  const spanCards = (
    <div className="agent-trace-page__timeline">
      {spans.map((span) => (
        <SpanCard
          key={span.id}
          span={span}
          spanRef={(node) => {
            if (node) nodeRefs.current.set(span.id, node);
            else nodeRefs.current.delete(span.id);
          }}
        />
      ))}
    </div>
  );

  const cardHeader = (
    <header className="agent-trace-breakdown__head">
      <h3 className="agent-trace-breakdown__title">Trace Breakdown</h3>
      <BreakdownViewToggle viewMode={viewMode} onChange={setViewMode} />
    </header>
  );

  return (
    <div className="agent-trace-details-layout">
      <aside className="agent-trace-breakdown" aria-label="Trace breakdown">
        {cardHeader}
        {viewMode === "execution-timeline" ? (
          <ExecutionTimelineGantt
            spans={spans}
            trace={traceMeta}
            selectedId={selectedId}
            onSelect={selectSpan}
          />
        ) : (
          <TraceBreakdownTree
            spans={spans}
            trace={traceMeta}
            selectedId={selectedId}
            onSelect={selectSpan}
          />
        )}
      </aside>
      <div className="agent-trace-details-layout__timeline">{spanCards}</div>
    </div>
  );
}

export function AgentTraceDetailPage() {
  const { traceId: rawTraceId } = useParams<{ traceId: string }>();
  const traceId = rawTraceId ? decodeURIComponent(rawTraceId) : "";
  const { backendConnected } = useAppStore();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<Awaited<
    ReturnType<typeof getAgentTraceDetail>
  > | null>(null);
  const [spansTab, setSpansTab] = useState<SpansTab>("summary");

  async function load() {
    if (!backendConnected || !traceId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const next = await getAgentTraceDetail(traceId);
      setDetail(next);
      if (!next) setError("Trace not found.");
    } catch (err) {
      setError(toAppError(err).message);
      setDetail(null);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, [backendConnected, traceId]);

  const spans = useMemo(() => {
    if (!detail) return [];
    return [...detail.spans].sort((a, b) =>
      a.startedAt.localeCompare(b.startedAt),
    );
  }, [detail]);

  const conv = conversationLabel(detail?.trace.sessionId);
  const sessionHref = detail?.trace.sessionId
    ? `/agent-trace?session=${encodeURIComponent(detail.trace.sessionId)}`
    : "/agent-trace";

  if (!backendConnected) {
    return (
      <div className="page agent-trace-detail">
        <PageHeader title="Agent Trace Detail" backTo="/agent-trace" backOnly />
        <Card>
          <EmptyState
            title="Backend offline"
            description="Connect the desktop backend to inspect this trace."
          />
        </Card>
      </div>
    );
  }

  if (loading && !detail) {
    return (
      <div className="page agent-trace-detail">
        <PageHeader title="Agent Trace Detail" backTo="/agent-trace" backOnly />
        <PageLoadingSkeleton />
      </div>
    );
  }

  if (!detail) {
    return (
      <div className="page agent-trace-detail">
        <PageHeader title="Agent Trace Detail" backTo="/agent-trace" backOnly />
        <EmptyState
          title="Trace not found"
          description={error ?? "This trace id is missing or was deleted."}
          action={
            <Link className="btn btn--secondary btn--sm" to="/agent-trace">
              Back to traces
            </Link>
          }
        />
      </div>
    );
  }

  const { trace } = detail;

  return (
    <div className="page agent-trace-detail">
      <PageHeader
        backTo="/agent-trace"
        backOnly
        title="Agent Trace Detail"
        description={trace.name}
        actions={
          <RefreshButton
            onClick={() => void load()}
            loading={loading}
            error={error}
            showSuccessToast={false}
          />
        }
      />

      {error ? (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      ) : null}

      <section
        className="project-details__overview"
        aria-label="Trace overview"
      >
        <Card className="detail-section project-details__meta">
          <h2 className="detail-section__title">Trace Information</h2>
          <div className="detail-section__body">
            <DetailRow
              label="Trace ID"
              value={<span className="mono text-sm">{trace.id}</span>}
            />
            <DetailRow
              label="Conversation ID"
              value={
                <Link className="link mono text-sm" to={sessionHref}>
                  {conv.id}
                </Link>
              }
            />
            <DetailRow
              label="Request Time"
              value={formatTraceTime(trace.startedAt)}
            />
            <DetailRow
              label="Ended"
              value={formatTraceTime(trace.endedAt)}
            />
          </div>
        </Card>

        <Card className="detail-section project-details__target-stats">
          <h2 className="detail-section__title">Trace Metrics</h2>
          <div className="detail-summary-grid detail-summary-grid--metrics">
            <SummaryStat
              label="Tokens"
              value={formatTokenCount(trace.totalTokens)}
            />
            <SummaryStat
              label="Execution Time"
              value={formatExecutionTime(trace.latencyMs)}
            />
            <SummaryStat label="Spans" value={spans.length} />
            <SummaryStat
              label="State"
              value={<StatusBadge status={stateLabel(trace.status)} />}
            />
          </div>
        </Card>
      </section>

      <section className="project-details__primary" aria-label="Spans">
        <Card className="detail-section agent-trace-detail__spans-card">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">Spans</h2>
              <p className="detail-section__hint">
                {spansTab === "summary"
                  ? "Input, output, and capability classification for this turn"
                  : "Span tree or execution timeline for this turn"}
              </p>
            </div>
            <div
              className="runtime-route-toggle runtime-route-toggle--header"
              role="tablist"
              aria-label="Span views"
            >
              <button
                type="button"
                role="tab"
                aria-selected={spansTab === "summary"}
                className={`runtime-route-toggle__btn${
                  spansTab === "summary"
                    ? " runtime-route-toggle__btn--active"
                    : ""
                }`}
                onClick={() => setSpansTab("summary")}
              >
                Summary
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={spansTab === "details"}
                className={`runtime-route-toggle__btn${
                  spansTab === "details"
                    ? " runtime-route-toggle__btn--active"
                    : ""
                }`}
                onClick={() => setSpansTab("details")}
              >
                Details & Timeline
              </button>
            </div>
          </div>

          {spans.length === 0 ? (
            <EmptyState
              title="No spans"
              description="This trace has no recorded spans."
            />
          ) : spansTab === "summary" ? (
            <TraceSummaryView
              spans={spans}
              traceLatencyMs={trace.latencyMs}
              traceTags={trace.tags}
            />
          ) : (
            <TraceDetailsView
              spans={spans}
              trace={{
                id: trace.id,
                name: trace.name,
                startedAt: trace.startedAt,
                endedAt: trace.endedAt,
                latencyMs: trace.latencyMs,
              }}
            />
          )}
        </Card>
      </section>
    </div>
  );
}
