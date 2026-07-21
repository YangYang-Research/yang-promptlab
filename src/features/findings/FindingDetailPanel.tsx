import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Badge, FindingStatusBadge, IconButton, SeverityBadge } from "@/shared/components";
import { IconCheck, IconCopy } from "@/shared/components/Icons";
import { getJudgeRoleWeights } from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";
import type { Finding, Severity } from "@/shared/types";

import {
  buildFindingCurl,
  DEFAULT_JUDGE_ROLE_WEIGHTS,
  parseFindingEvidence,
  type JudgeRoleResult,
  type JudgeRoleWeights,
  type JudgeScoreBreakdown,
  type ParsedFindingEvidence,
} from "./findingEvidence";

type FindingDetailPanelProps = {
  finding: Finding;
  onClose?: () => void;
  /** Hide title header when the parent page already shows it. */
  embedded?: boolean;
  /** Which evidence sections to render. */
  mode?: "all" | "poc" | "judge";
};

export function FindingDetailPanel({
  finding,
  onClose,
  embedded = false,
  mode = "all",
}: FindingDetailPanelProps) {
  const [roleWeights, setRoleWeights] = useState<JudgeRoleWeights>(DEFAULT_JUDGE_ROLE_WEIGHTS);

  useEffect(() => {
    let cancelled = false;
    void getJudgeRoleWeights()
      .then((weights) => {
        if (cancelled) return;
        setRoleWeights({
          judge: weights.judge,
          classifier: weights.classifier,
          attacker: weights.attacker,
          defaultLlm: weights.defaultLlm,
        });
      })
      .catch(() => {
        /* keep defaults when offline / mock */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const evidence = useMemo(
    () => parseFindingEvidence(finding, roleWeights),
    [finding, roleWeights],
  );
  const showPoc = mode === "all" || mode === "poc";
  const showJudge = mode === "all" || mode === "judge";
  const hasTraffic =
    Boolean(evidence.requestBody) ||
    Boolean(evidence.requestUrl) ||
    Boolean(evidence.responseBody) ||
    Boolean(evidence.responseExcerpt) ||
    evidence.responseStatus != null;
  const hasPayload = Boolean(evidence.payload?.trim());
  const hasJudge =
    evidence.judgeRoles.length > 0 ||
    evidence.indicators.length > 0 ||
    Boolean(evidence.judgeSummary || evidence.judgeReasoning);
  const hasPoc = hasTraffic || hasPayload;

  return (
    <section className={`finding-detail ${embedded ? "finding-detail--embedded" : ""}`}>
      {!embedded && (
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
      )}

      {showPoc && hasPoc && (
        <div className="finding-detail__evidence-stack">
          {hasPayload && (
            <EvidenceBlock
              title="Payload"
              subtitle={evidence.payloadId ?? undefined}
              content={evidence.payload}
              wide
              actions={<CopyPayloadButton payload={evidence.payload} />}
            />
          )}

          {hasTraffic && (
            <div className="finding-detail__grid finding-detail__grid--traffic">
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
                actions={<CopyCurlButton evidence={evidence} />}
              />
              <EvidenceBlock
                title="Response"
                subtitle={responseStatusSubtitle(evidence.requestUrl, evidence.responseStatus)}
                content={evidence.responseBody ?? evidence.responseExcerpt}
              />
            </div>
          )}
        </div>
      )}

      {showJudge && hasJudge && (
        <JudgeAnalysisSection evidence={evidence} showTitle={mode === "all"} />
      )}

      {((showPoc && !hasPoc && !showJudge) ||
        (showJudge && !hasJudge && !showPoc) ||
        (mode === "all" && !hasPoc && !hasJudge)) && (
        <p className="finding-detail__empty text-muted">
          {mode === "judge"
            ? "No judge analysis is attached to this finding."
            : mode === "poc"
              ? "No proof-of-concept evidence is attached to this finding."
              : "No structured evidence is attached to this finding."}
        </p>
      )}

      {finding.verdict && !embedded && (
        <div className="finding-detail__footer">
          <Badge variant={finding.verdict === "vulnerable" ? "danger" : "muted"}>
            {finding.verdict === "vulnerable" ? "Vulnerable" : "Not vulnerable"}
          </Badge>
          <FindingStatusBadge status={finding.status} />
        </div>
      )}
    </section>
  );
}

function JudgeAnalysisSection({
  evidence,
  showTitle = true,
}: {
  evidence: ParsedFindingEvidence;
  showTitle?: boolean;
}) {
  const hasRoles = evidence.judgeRoles.length > 0;
  const indicatorRows = buildIndicatorRows(evidence);
  const breakdown = evidence.judgeScoreBreakdown;
  const displayScore = breakdown?.finalScore ?? evidence.judgeScore;

  return (
    <article
      className={`finding-detail__block finding-detail__block--wide finding-detail__judge${
        showTitle ? "" : " finding-detail__judge--untitled"
      }`}
    >
      {showTitle && (
        <div className="finding-detail__block-header finding-detail__judge-header">
          <h4>Judge analysis</h4>
        </div>
      )}

      {hasRoles ? (
        <div className="finding-detail__judge-roles">
          {evidence.judgeRoles.map((role) => (
            <JudgeRoleCard key={role.id} role={role} />
          ))}
        </div>
      ) : (
        (evidence.judgeSummary || evidence.judgeReasoning) && (
          <pre className="finding-detail__code">
            {[evidence.judgeSummary, evidence.judgeReasoning].filter(Boolean).join("\n\n") || "—"}
          </pre>
        )
      )}

      {indicatorRows.length > 0 && <IndicatorsTable rows={indicatorRows} />}

      {(displayScore != null || evidence.verdict || breakdown) && (
        <JudgeScoreSummary
          score={displayScore}
          verdict={evidence.verdict}
          breakdown={breakdown}
        />
      )}
    </article>
  );
}

function JudgeScoreSummary({
  score,
  verdict,
  breakdown,
}: {
  score: number | null;
  verdict: string | null;
  breakdown: JudgeScoreBreakdown | null;
}) {
  return (
    <div className="finding-detail__score-summary">
      <div className="finding-detail__score-summary-head">
        <span className="finding-detail__score-label--heading">Score</span>
        <div className="finding-detail__score-summary-metrics">
          {score != null && (
            <span className="finding-detail__score finding-detail__score--summary">
              <span className="finding-detail__score-value">{score}</span>
              <span className="finding-detail__score-max">/100</span>
            </span>
          )}
          {verdict && (
            <Badge variant={verdict === "vulnerable" ? "danger" : "muted"}>
              {verdict === "vulnerable" ? "Vulnerable" : "Not vulnerable"}
            </Badge>
          )}
        </div>
      </div>

      {breakdown && (
        <div className="finding-detail__score-formula">
          <p className="finding-detail__score-formula-title">Score formula</p>
          <p className="finding-detail__score-formula-eq mono">
            score = min(1, Σ(confidence × weight) / Σ(weight) + agreement_boost)
          </p>
          <div className="finding-detail__score-formula-table-wrap">
            <table className="finding-detail__score-formula-table">
              <thead>
                <tr>
                  <th scope="col">Role</th>
                  <th scope="col">Confidence</th>
                  <th scope="col">Weight</th>
                  <th scope="col">conf × w</th>
                </tr>
              </thead>
              <tbody>
                {breakdown.terms.map((term) => (
                  <tr key={`${term.role}-${term.label}`}>
                    <td>{term.label}</td>
                    <td className="mono">{(term.confidence * 100).toFixed(0)}%</td>
                    <td className="mono">{term.weight.toFixed(2)}</td>
                    <td className="mono">{term.weighted.toFixed(3)}</td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr>
                  <td>Σ</td>
                  <td />
                  <td className="mono">{breakdown.weightTotal.toFixed(2)}</td>
                  <td className="mono">{breakdown.weightedSum.toFixed(3)}</td>
                </tr>
              </tfoot>
            </table>
          </div>
          <ul className="finding-detail__score-formula-steps">
            <li>
              <span className="finding-detail__score-step-label">Base</span>
              <span className="mono">
                {breakdown.weightedSum.toFixed(3)} / {breakdown.weightTotal.toFixed(2)} ={" "}
                {(breakdown.base * 100).toFixed(1)}%
              </span>
            </li>
            <li>
              <span className="finding-detail__score-step-label">Agreement</span>
              <span className="mono">
                {breakdown.vulnerableCount}/{breakdown.totalCount} vulnerable (
                {(breakdown.agreement * 100).toFixed(0)}%)
              </span>
            </li>
            <li>
              <span className="finding-detail__score-step-label">Boost</span>
              <span className="mono">
                {breakdown.boost === 0
                  ? "+0 (agreement < 50%)"
                  : breakdown.boost >= 0.08
                    ? "+0.08 (≥66% agree vulnerable)"
                    : "+0.04 (≥50% agree vulnerable)"}
              </span>
            </li>
            <li>
              <span className="finding-detail__score-step-label">Final</span>
              <span className="mono">
                min(1, {(breakdown.base * 100).toFixed(1)}% + {(breakdown.boost * 100).toFixed(0)}%) ={" "}
                {breakdown.finalScore}/100
              </span>
            </li>
          </ul>
        </div>
      )}
    </div>
  );
}

type IndicatorRow = {
  id: string;
  index: number;
  indicator: string;
  role: string | null;
};

function buildIndicatorRows(evidence: ParsedFindingEvidence): IndicatorRow[] {
  const fromRoles = evidence.judgeRoles.flatMap((role) =>
    role.indicators.map((indicator) => ({
      indicator,
      role: role.label,
    })),
  );

  const source =
    fromRoles.length > 0
      ? fromRoles
      : evidence.indicators.map((indicator) => ({ indicator, role: null as string | null }));

  const seen = new Set<string>();
  const rows: IndicatorRow[] = [];
  for (const item of source) {
    const key = `${item.role ?? ""}::${item.indicator}`;
    if (seen.has(key)) continue;
    seen.add(key);
    rows.push({
      id: key,
      index: rows.length + 1,
      indicator: item.indicator,
      role: item.role,
    });
  }
  return rows;
}

function IndicatorsTable({ rows }: { rows: IndicatorRow[] }) {
  const showRole = rows.some((row) => row.role);

  return (
    <div className="finding-detail__indicators-wrap">
      <h5 className="finding-detail__indicators-title">Indicators</h5>
      <div className="finding-detail__indicators-table-wrap">
        <table className="finding-detail__indicators-table">
          <thead>
            <tr>
              <th scope="col" className="finding-detail__indicators-col-index">
                #
              </th>
              {showRole && <th scope="col">Role</th>}
              <th scope="col">Indicator</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.id}>
                <td className="finding-detail__indicators-col-index mono">{row.index}</td>
                {showRole && <td className="finding-detail__indicators-col-role">{row.role ?? "—"}</td>}
                <td>{row.indicator}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function JudgeRoleCard({ role }: { role: JudgeRoleResult }) {
  const severity = coerceSeverity(role.severity);

  return (
    <div className={`finding-detail__role finding-detail__role--${role.role}`}>
      <div className="finding-detail__role-header">
        <div className="finding-detail__role-title">
          <span className="finding-detail__role-name">{role.label}</span>
        </div>
        <div className="finding-detail__role-badges">
          <span className="finding-detail__score finding-detail__score--role">
            <span className="finding-detail__score-value">{role.score}</span>
            <span className="finding-detail__score-max">/100</span>
          </span>
          <Badge variant={role.vulnerable ? "danger" : "muted"}>
            {role.vulnerable ? "Vulnerable" : "Not vulnerable"}
          </Badge>
          {severity && <SeverityBadge severity={severity} />}
        </div>
      </div>

      {role.category && (
        <p className="finding-detail__role-category text-sm text-muted">
          Category: {role.category.replace(/_/g, " ")}
        </p>
      )}

      {role.rationale && <p className="finding-detail__role-rationale">{role.rationale}</p>}
    </div>
  );
}

function coerceSeverity(value: string | null): Severity | null {
  if (!value) return null;
  const key = value.toLowerCase();
  if (
    key === "critical" ||
    key === "high" ||
    key === "medium" ||
    key === "low" ||
    key === "info"
  ) {
    return key;
  }
  return null;
}

function responseStatusSubtitle(
  requestUrl: string | null,
  status: number | null,
): ReactNode | undefined {
  if (status == null) return undefined;
  const scheme = requestScheme(requestUrl);
  const statusClass = statusCodeClass(status);
  const statusBadge = (
    <span className={`finding-detail__status-code ${statusClass}`}>{status}</span>
  );
  if (!scheme) return statusBadge;
  return (
    <span className="finding-detail__status-line">
      <span className={`finding-detail__scheme finding-detail__scheme--${scheme}`}>
        {scheme.toUpperCase()}
      </span>
      {statusBadge}
    </span>
  );
}

function statusCodeClass(status: number): string {
  if (status >= 200 && status < 300) return "finding-detail__status-code--2xx";
  if (status >= 300 && status < 400) return "finding-detail__status-code--3xx";
  if (status >= 400 && status < 500) return "finding-detail__status-code--4xx";
  if (status >= 500) return "finding-detail__status-code--5xx";
  return "";
}

function requestScheme(requestUrl: string | null): string | null {
  if (!requestUrl) return null;
  try {
    const scheme = new URL(requestUrl).protocol.replace(/:$/, "").toLowerCase();
    return scheme || null;
  } catch {
    const match = requestUrl.match(/^([a-z][a-z0-9+.-]*):\/\//i);
    return match?.[1]?.toLowerCase() ?? null;
  }
}

function CopyPayloadButton({ payload }: { payload: string | null }) {
  const { notify } = useToast();
  const [copied, setCopied] = useState(false);
  const text = payload?.trim();
  if (!text) return null;

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      notify("Payload copied", "success");
      window.setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      notify(error instanceof Error ? error.message : "Failed to copy payload", "error");
    }
  }

  return (
    <IconButton
      ariaLabel={copied ? "Payload copied" : "Copy payload"}
      size="sm"
      active={copied}
      onClick={() => void handleCopy()}
    >
      {copied ? <IconCheck /> : <IconCopy />}
    </IconButton>
  );
}

function CopyCurlButton({ evidence }: { evidence: ParsedFindingEvidence }) {
  const { notify } = useToast();
  const [copied, setCopied] = useState(false);
  const curl = buildFindingCurl(evidence);

  if (!curl) return null;

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(curl);
      setCopied(true);
      notify("cURL copied", "success");
      window.setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      notify(error instanceof Error ? error.message : "Failed to copy cURL", "error");
    }
  }

  return (
    <IconButton
      ariaLabel={copied ? "cURL copied" : "Copy cURL"}
      size="sm"
      active={copied}
      onClick={() => void handleCopy()}
    >
      {copied ? <IconCheck /> : <IconCopy />}
    </IconButton>
  );
}

function EvidenceBlock({
  title,
  subtitle,
  content,
  actions,
  wide = false,
}: {
  title: string;
  subtitle?: ReactNode;
  content: string | null;
  actions?: ReactNode;
  wide?: boolean;
}) {
  const empty = !content?.trim();

  return (
    <article
      className={`finding-detail__block${wide ? " finding-detail__block--wide" : ""}${
        empty ? " finding-detail__block--empty" : ""
      }`}
    >
      <div className="finding-detail__block-header">
        <div className="finding-detail__block-title-row">
          <h4>{title}</h4>
          {actions}
        </div>
        {subtitle ? <span className="finding-detail__block-sub mono">{subtitle}</span> : null}
      </div>
      <pre className="finding-detail__code">{empty ? "—" : content}</pre>
    </article>
  );
}
