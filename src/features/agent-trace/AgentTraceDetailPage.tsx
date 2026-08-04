import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
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
  shortId,
  stateLabel,
} from "./traceFormat";

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
            <span className="agent-trace-payload__stage">inputs</span>
          </header>
          <PayloadBody
            text={span.inputs != null ? prettyJson(span.inputs) : "—"}
            defaultOpen
          />
        </section>
        <section className="agent-trace-payload agent-trace-payload--res">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">Out</span>
            <span className="agent-trace-payload__stage">outputs</span>
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
  const navigate = useNavigate();
  const { backendConnected } = useAppStore();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<Awaited<
    ReturnType<typeof getAgentTraceDetail>
  > | null>(null);

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
      <div className="agent-trace-detail page">
        <PageHeader title="Agent Trace Detail" />
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
      <div className="agent-trace-detail page">
        <PageHeader title="Agent Trace Detail" />
        <PageLoadingSkeleton />
      </div>
    );
  }

  if (!detail) {
    return (
      <div className="agent-trace-detail page">
        <PageHeader
          title="Agent Trace Detail"
          actions={
            <Button variant="secondary" size="sm" onClick={() => navigate("/agent-trace")}>
              Back to traces
            </Button>
          }
        />
        <Card>
          <EmptyState
            title="Trace not found"
            description={error ?? "This trace id is missing or was deleted."}
            action={
              <Link className="btn btn--secondary btn--sm" to="/agent-trace">
                Back to traces
              </Link>
            }
          />
        </Card>
      </div>
    );
  }

  const { trace } = detail;

  return (
    <div className="agent-trace-detail page">
      <PageHeader
        title="Agent Trace Detail"
        description={trace.name}
        actions={
          <div className="page-actions">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => navigate("/agent-trace")}
            >
              Back to traces
            </Button>
            <RefreshButton
              onClick={() => void load()}
              loading={loading}
              error={error}
              showSuccessToast={false}
            />
          </div>
        }
      />

      {error ? (
        <Card className="agent-trace-page__banner agent-trace-page__banner--error">
          <p className="agent-trace-page__error">{error}</p>
        </Card>
      ) : null}

      <Card className="agent-trace-detail__summary">
        <dl className="agent-trace-detail__meta">
          <div>
            <dt>Trace ID</dt>
            <dd className="mono" title={trace.id}>
              {trace.id}
            </dd>
          </div>
          <div>
            <dt>Conversation ID</dt>
            <dd>
              <Link className="agent-trace-page__info-link" to={sessionHref}>
                <span className="agent-trace-detail__conv-title">{conv.title}</span>
                <code className="mono text-sm" title={trace.sessionId ?? undefined}>
                  {shortId(conv.id, 14)}
                </code>
              </Link>
            </dd>
          </div>
          <div>
            <dt>Tokens</dt>
            <dd className="mono agent-trace-page__num">
              {formatTokenCount(trace.totalTokens)}
            </dd>
          </div>
          <div>
            <dt>Execution time</dt>
            <dd className="mono agent-trace-page__num">
              {formatExecutionTime(trace.latencyMs)}
            </dd>
          </div>
          <div>
            <dt>State</dt>
            <dd>
              <StatusBadge status={stateLabel(trace.status)} />
            </dd>
          </div>
          <div>
            <dt>Started</dt>
            <dd className="mono text-sm">{formatTraceTime(trace.startedAt)}</dd>
          </div>
          <div>
            <dt>Ended</dt>
            <dd className="mono text-sm">{formatTraceTime(trace.endedAt)}</dd>
          </div>
          <div>
            <dt>Spans</dt>
            <dd className="mono agent-trace-page__num">{spans.length}</dd>
          </div>
        </dl>
      </Card>

      <section className="agent-trace-detail__spans">
        <header className="agent-trace-detail__spans-head">
          <div>
            <p className="agent-trace-page__eyebrow">Span tree</p>
            <h3 className="agent-trace-detail__spans-title">
              Capability · LLM · tools
            </h3>
          </div>
          <span className="agent-trace-page__metric">{spans.length}</span>
        </header>

        {spans.length === 0 ? (
          <EmptyState
            title="No spans"
            description="This trace has no recorded spans."
          />
        ) : (
          <div className="agent-trace-page__timeline">
            {spans.map((span) => (
              <SpanCard key={span.id} span={span} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
