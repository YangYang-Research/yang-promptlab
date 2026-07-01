import { Badge, SeverityBadge } from "@/shared/components";
import type { Finding } from "@/shared/types";

import { parseFindingEvidence } from "./findingEvidence";

type FindingDetailPanelProps = {
  finding: Finding;
  onClose?: () => void;
};

export function FindingDetailPanel({ finding, onClose }: FindingDetailPanelProps) {
  const evidence = parseFindingEvidence(finding);

  return (
    <section className="finding-detail">
      <header className="finding-detail__header">
        <div>
          <div className="finding-detail__title-row">
            <SeverityBadge severity={finding.severity} />
            <h3 className="finding-detail__title">{finding.title}</h3>
          </div>
          <p className="text-sm text-muted">
            {finding.category.replace(/_/g, " ")} · {finding.targetName || "target"} ·{" "}
            {new Date(finding.discoveredAt).toLocaleString()}
          </p>
        </div>
        {onClose ? (
          <button type="button" className="finding-detail__close" onClick={onClose}>
            Close
          </button>
        ) : null}
      </header>

      {evidence.explanation && (
        <div className="finding-detail__section">
          <h4>Explanation</h4>
          <p>{evidence.explanation}</p>
          {evidence.confidence != null && (
            <p className="text-sm text-muted">
              Confidence {Math.round(evidence.confidence * 100)}%
              {evidence.verdict ? ` · ${evidence.verdict}` : ""}
            </p>
          )}
        </div>
      )}

      {evidence.indicators.length > 0 && (
        <div className="finding-detail__section">
          <h4>Indicators</h4>
          <ul className="finding-detail__indicators">
            {evidence.indicators.map((indicator) => (
              <li key={indicator}>{indicator}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="finding-detail__grid">
        <EvidenceBlock
          title="Request"
          subtitle={
            evidence.requestMethod || evidence.requestUrl
              ? [evidence.requestMethod, evidence.requestUrl].filter(Boolean).join(" ")
              : undefined
          }
          content={
            evidence.requestBody ??
            (evidence.requestUrl
              ? `${evidence.requestMethod ?? "POST"} ${evidence.requestUrl}`
              : null)
          }
        />
        <EvidenceBlock
          title="Payload"
          subtitle={evidence.payloadId ?? undefined}
          content={evidence.payload}
        />
        <EvidenceBlock
          title="Response"
          subtitle={
            evidence.responseStatus != null ? `HTTP ${evidence.responseStatus}` : undefined
          }
          content={evidence.responseBody ?? evidence.responseExcerpt}
        />
        {(evidence.judgeSummary || evidence.judgeReasoning) && (
          <EvidenceBlock
            title="Judge analysis"
            content={[evidence.judgeSummary, evidence.judgeReasoning].filter(Boolean).join("\n\n")}
          />
        )}
      </div>

      {finding.verdict && (
        <div className="finding-detail__footer">
          <Badge variant={finding.verdict === "vulnerable" ? "danger" : "muted"}>
            {finding.verdict === "vulnerable" ? "Vulnerable" : "Not vulnerable"}
          </Badge>
          <Badge variant="muted">{finding.status.replace(/_/g, " ")}</Badge>
        </div>
      )}
    </section>
  );
}

function EvidenceBlock({
  title,
  subtitle,
  content,
}: {
  title: string;
  subtitle?: string;
  content: string | null;
}) {
  return (
    <div className="finding-detail__block">
      <div className="finding-detail__block-header">
        <h4>{title}</h4>
        {subtitle ? <span className="text-sm text-muted mono">{subtitle}</span> : null}
      </div>
      <pre className="finding-detail__code">
        {content?.trim() ? content : "—"}
      </pre>
    </div>
  );
}
