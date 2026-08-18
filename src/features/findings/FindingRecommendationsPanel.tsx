import { useEffect, useRef, useState } from "react";

import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { Badge, Button, YazgBadge } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import {
  generateFindingRecommendations,
  type FindingRecommendationDto,
} from "@/shared/ipc/findingRecommendations";
import type { Finding } from "@/shared/types";

function recommendationPriorityVariant(
  priority: string,
): "danger" | "warning" | "info" | "muted" {
  const normalized = priority.trim().toLowerCase();
  if (normalized === "critical" || normalized === "high") return "danger";
  if (normalized === "medium") return "warning";
  if (normalized === "low") return "info";
  return "muted";
}

type FindingRecommendationsPanelProps = {
  finding: Pick<Finding, "id" | "category" | "severity" | "title" | "status" | "description">;
  /** `section` includes the Recommendations heading (Finding Details). */
  variant?: "section" | "embedded";
  /** Heading for `embedded` variant (default: Recommendation). */
  embeddedTitle?: string;
  className?: string;
  /** When false, hide the Re-recommend action (e.g. report Detailed Findings). */
  showReRecommend?: boolean;
  enabled?: boolean;
  /**
   * Queue priority for Report Details (list index, top = 0).
   * Lower values run first under the shared finding-recommend queue.
   */
  queueOrder?: number;
};

export function FindingRecommendationsPanel({
  finding,
  variant = "section",
  embeddedTitle = "Recommendation",
  className,
  showReRecommend = true,
  enabled = true,
  queueOrder = 0,
}: FindingRecommendationsPanelProps) {
  const [recommendations, setRecommendations] = useState<FindingRecommendationDto[]>([]);
  const [overview, setOverview] = useState<string | null>(null);
  const [source, setSource] = useState<"ai" | "fallback" | string | null>(null);
  const [generatedAt, setGeneratedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);
  const requestIdRef = useRef(0);

  const revision = [
    finding.id,
    finding.title,
    finding.severity,
    finding.category,
    finding.status,
    finding.description ?? "",
  ].join("|");

  const load = (force: boolean) => {
    if (!enabled || !finding.id) return;
    const requestId = ++requestIdRef.current;
    const fetchKey = revision;
    setLoading(true);
    setError(null);

    void generateFindingRecommendations(finding.id, { force, order: queueOrder })
      .then((response) => {
        if (requestId !== requestIdRef.current) return;
        fetchedRef.current = fetchKey;
        setRecommendations(response.recommendations);
        setOverview(response.overview?.trim() || null);
        setSource(response.source);
        setGeneratedAt(response.generated_at?.trim() || null);
      })
      .catch((err) => {
        if (requestId !== requestIdRef.current) return;
        if (!force) fetchedRef.current = null;
        setRecommendations([]);
        setOverview(null);
        setSource(null);
        setGeneratedAt(null);
        setError(err instanceof Error ? err.message : "Failed to load recommendations");
      })
      .finally(() => {
        if (requestId === requestIdRef.current) setLoading(false);
      });
  };

  useEffect(() => {
    if (!enabled || !finding.id) return;
    if (fetchedRef.current === revision) return;

    const timer = window.setTimeout(() => {
      if (fetchedRef.current === revision) return;
      load(false);
    }, 0);

    return () => {
      window.clearTimeout(timer);
      requestIdRef.current += 1;
    };
  }, [enabled, finding.id, revision]);

  if (!enabled) return null;

  const empty = recommendations.length === 0 && !overview;
  const sourceBadge =
    source === "ai" ? (
      <YazgBadge pulsing={loading} />
    ) : source === "fallback" ? (
      <Badge variant="muted">Rule-based guidance</Badge>
    ) : null;

  const list = (
    <ol className="scan-rec__list finding-rec__list">
      {recommendations.map((item, index) => (
        <li key={`${item.title}-${item.priority}-${index}`} className="scan-rec__item">
          <span className="scan-rec__index" aria-hidden="true">
            {String(index + 1).padStart(2, "0")}
          </span>
          <div className="scan-rec__body">
            <div className="scan-rec__item-head">
              <h3 className="scan-rec__item-title">{item.title}</h3>
              <Badge variant={recommendationPriorityVariant(item.priority)}>{item.priority}</Badge>
            </div>
            <p className="scan-rec__item-desc">{item.description}</p>
          </div>
        </li>
      ))}
    </ol>
  );

  const footer =
    generatedAt || showReRecommend ? (
      <div className="project-summary__footer">
        {generatedAt ? (
          <p className="project-summary__generated">Generated {formatTimestamp(generatedAt)}</p>
        ) : (
          <span />
        )}
        {showReRecommend ? (
          <Button
            variant="primary"
            size="sm"
            type="button"
            className="project-summary__action"
            onClick={() => load(true)}
            disabled={loading}
          >
            <span className="btn__content">
              <IconAi className="btn__icon" aria-hidden />
              {loading ? "Re-recommending…" : "Re-recommend"}
            </span>
          </Button>
        ) : null}
      </div>
    ) : null;

  const body = loading && empty ? (
    <div className="scan-rec__loading" aria-busy="true" aria-live="polite">
      <div className="scan-rec__skeleton scan-rec__skeleton--lead" />
      <div className="scan-rec__skeleton" />
      <div className="scan-rec__skeleton scan-rec__skeleton--short" />
      <p className="scan-rec__status text-muted text-sm">
        Yazg is generating remediation recommendations for this finding…
      </p>
    </div>
  ) : error && empty ? (
    <p className="scan-rec__status text-danger text-sm">{error}</p>
  ) : empty ? (
    <p className="scan-rec__status text-muted text-sm">No recommendations available yet.</p>
  ) : (
    <>
      {error ? <p className="scan-rec__status text-danger text-sm">{error}</p> : null}
      {overview ? <p className="scan-rec__overview">{overview}</p> : null}
      {list}
      {footer}
    </>
  );

  if (variant === "embedded") {
    return (
      <div
        className={["finding-rec", "finding-rec--embedded", className].filter(Boolean).join(" ")}
        data-state={loading && empty ? "loading" : error ? "error" : empty ? "empty" : "ready"}
      >
        <header className="finding-rec__embedded-header">
          <span className="report-native__evidence-label finding-rec__embedded-title">
            {embeddedTitle}
          </span>
          {sourceBadge ? <div className="finding-rec__embedded-badge">{sourceBadge}</div> : null}
        </header>
        {body}
      </div>
    );
  }

  return (
    <div
      className={["scan-rec", "finding-rec", className].filter(Boolean).join(" ")}
      data-state={loading && empty ? "loading" : error ? "error" : empty ? "empty" : "ready"}
    >
      <header className="scan-rec__header">
        <h2 className="scan-rec__title">Recommendations</h2>
        {sourceBadge}
      </header>
      {body}
    </div>
  );
}
