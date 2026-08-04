import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Card,
  EmptyState,
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
}: {
  text: string;
  defaultOpen?: boolean;
}) {
  const long = text.length > 480 || text.split("\n").length > 12;
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

function kindClass(kind: string): string {
  const k = kind.toLowerCase();
  if (k.includes("capability")) return "capability";
  if (k === "llm") return "llm";
  if (k.includes("tool")) return "tool";
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

  return (
    <div className="agent-trace-summary-folds">
      <SummaryFold title="Input" hint="User message">
        <div className="agent-trace-summary-fold__stack">
          <SummaryField label="Message">{summary.input}</SummaryField>
          <SummaryField label="Turn">
            {summary.turn != null ? String(summary.turn) : "—"}
          </SummaryField>
          <details className="agent-trace-io-fold">
            <summary className="agent-trace-io-fold__summary">
              <span className="agent-trace-io-fold__fn mono">
                {summary.wireFunctionName}
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
      <SummaryFold title="Output" hint="Assistant reply">
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

function SpanCard({ span }: { span: AgentTraceSpanDto }) {
  const family = kindClass(span.kind);
  const metrics = Object.entries(span.metrics ?? {});
  return (
    <article className={`agent-trace-group agent-trace-group--${family}`}>
      <header className="agent-trace-group__head">
        <span
          className={`agent-trace-group__family agent-trace-group__family--${family}`}
        >
          {span.kind}
        </span>
        <span className="agent-trace-group__stages">{span.name}</span>
        <span className="agent-trace-group__time">
          {span.latencyMs != null ? `${span.latencyMs} ms · ` : ""}
          {formatTraceTime(span.startedAt)}
        </span>
      </header>
      <div className="agent-trace-group__grid">
        <section className="agent-trace-payload agent-trace-payload--req">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">In</span>
            <span className="agent-trace-payload__stage">Inputs</span>
          </header>
          <PayloadBody
            text={span.inputs != null ? prettyJson(span.inputs) : "—"}
            defaultOpen
          />
        </section>
        <section className="agent-trace-payload agent-trace-payload--res">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">Out</span>
            <span className="agent-trace-payload__stage">Outputs</span>
            <span className="agent-trace-payload__meta">{span.status}</span>
          </header>
          <PayloadBody
            text={span.outputs != null ? prettyJson(span.outputs) : "—"}
            defaultOpen
          />
        </section>
      </div>
      {metrics.length > 0 ? (
        <dl className="agent-trace-detail__span-metrics">
          {metrics.map(([key, value]) => (
            <div key={key}>
              <dt>{key}</dt>
              <dd>{Number.isInteger(value) ? value : value.toFixed(2)}</dd>
            </div>
          ))}
        </dl>
      ) : null}
    </article>
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
                  : "Full inputs, outputs, and timeline for this turn"}
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
            <div className="agent-trace-page__timeline">
              {spans.map((span) => (
                <SpanCard key={span.id} span={span} />
              ))}
            </div>
          )}
        </Card>
      </section>
    </div>
  );
}
