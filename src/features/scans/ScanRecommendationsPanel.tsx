import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  buildScanRetryUrl,
  buildScanStartAttackUrl,
} from "@/features/scans/wizardState";
import { Badge, Button, YazgBadge } from "@/shared/components";
import {
  generateScanRecommendations,
  type AttackRecommendationDto,
} from "@/shared/ipc/scanRecommendations";

function recommendationPriorityVariant(
  priority: string,
): "danger" | "warning" | "info" | "muted" {
  if (priority === "critical" || priority === "high") return "danger";
  if (priority === "medium") return "warning";
  if (priority === "low") return "info";
  return "muted";
}

function isActionRecommendation(item: AttackRecommendationDto): boolean {
  const action = item.action?.trim().toLowerCase();
  return action === "retry_scan" || action === "start_attack";
}

type ScanRecommendationsPanelProps = {
  scanId: string;
  attackCategories?: string[];
  /** When false, skip loading (e.g. non-attack / draft scans). */
  enabled?: boolean;
  className?: string;
  /** `details` uses the Scan Details editorial layout; `wizard` keeps the compact step-6 style. */
  variant?: "wizard" | "details";
  /** Required for Retry Scan / Start Attack navigation from Scan Details. */
  projectId?: string;
  targetId?: string | null;
  /** In-wizard overrides (avoid full navigation). */
  onRetryScan?: () => void;
  onStartAttack?: () => void;
};

export function ScanRecommendationsPanel({
  scanId,
  attackCategories = [],
  enabled = true,
  className,
  variant = "wizard",
  projectId,
  targetId,
  onRetryScan,
  onStartAttack,
}: ScanRecommendationsPanelProps) {
  const navigate = useNavigate();
  const [recommendations, setRecommendations] = useState<AttackRecommendationDto[]>([]);
  const [overview, setOverview] = useState<string | null>(null);
  const [source, setSource] = useState<"ai" | "fallback" | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  useEffect(() => {
    if (!enabled || !scanId) return;
    if (fetchedRef.current === scanId) return;

    let cancelled = false;
    setLoading(true);
    setError(null);

    void generateScanRecommendations(scanId, attackCategories)
      .then((response) => {
        if (cancelled) return;
        fetchedRef.current = scanId;
        setRecommendations(response.recommendations);
        setOverview(response.overview?.trim() || null);
        setSource(response.source);
      })
      .catch((err) => {
        if (cancelled) return;
        fetchedRef.current = null;
        setRecommendations([]);
        setOverview(null);
        setSource(null);
        setError(err instanceof Error ? err.message : "Failed to load recommendations");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [scanId, enabled, attackCategories.join("|")]);

  function handleRetryScan() {
    if (onRetryScan) {
      onRetryScan();
      return;
    }
    if (!projectId) return;
    navigate(buildScanRetryUrl(projectId, scanId, targetId));
  }

  function handleStartAttack() {
    if (onStartAttack) {
      onStartAttack();
      return;
    }
    if (!projectId) return;
    navigate(buildScanStartAttackUrl(projectId, scanId, targetId));
  }

  function renderActionButtons(item: AttackRecommendationDto) {
    if (!isActionRecommendation(item)) return null;
    if (!onRetryScan && !onStartAttack && !projectId) return null;
    return (
      <div className="scan-rec__actions">
        <Button variant="primary" size="sm" onClick={handleRetryScan}>
          Retry Scan
        </Button>
        <Button variant="secondary" size="sm" onClick={handleStartAttack}>
          Start Attack
        </Button>
      </div>
    );
  }

  if (!enabled) return null;

  const empty = recommendations.length === 0 && !overview;
  const sourceBadge =
    source === "ai" ? (
      <YazgBadge />
    ) : source === "fallback" ? (
      <Badge variant="muted">Rule-based guidance</Badge>
    ) : null;

  if (variant === "details") {
    return (
      <div
        className={["scan-rec", className].filter(Boolean).join(" ")}
        data-state={loading && empty ? "loading" : error ? "error" : empty ? "empty" : "ready"}
      >
        <header className="scan-rec__header">
          <h2 className="scan-rec__title">Recommendations</h2>
          {sourceBadge}
        </header>

        {loading && empty ? (
          <div className="scan-rec__loading" aria-busy="true" aria-live="polite">
            <div className="scan-rec__skeleton scan-rec__skeleton--lead" />
            <div className="scan-rec__skeleton" />
            <div className="scan-rec__skeleton scan-rec__skeleton--short" />
            <p className="scan-rec__status text-muted text-sm">
              Yazg is generating recommendations from findings…
            </p>
          </div>
        ) : error ? (
          <p className="scan-rec__status text-danger text-sm">{error}</p>
        ) : empty ? (
          <p className="scan-rec__status text-muted text-sm">
            No recommendations available yet.
          </p>
        ) : (
          <>
            {overview ? (
              <p className="scan-rec__overview">{overview}</p>
            ) : null}

            <ol className="scan-rec__list">
              {recommendations.map((item, index) => (
                <li key={`${item.title}-${item.priority}-${index}`} className="scan-rec__item">
                  <span className="scan-rec__index" aria-hidden="true">
                    {String(index + 1).padStart(2, "0")}
                  </span>
                  <div className="scan-rec__body">
                    <div className="scan-rec__item-head">
                      <h3 className="scan-rec__item-title">{item.title}</h3>
                      <Badge variant={recommendationPriorityVariant(item.priority)}>
                        {item.priority}
                      </Badge>
                    </div>
                    <p className="scan-rec__item-desc">{item.description}</p>
                    {renderActionButtons(item)}
                  </div>
                </li>
              ))}
            </ol>
          </>
        )}
      </div>
    );
  }

  return (
    <div className={["wizard-results__recommendations", className].filter(Boolean).join(" ")}>
      <div className="wizard-results__recommendations-header">
        <h4 className="wizard-results__recommendations-title">Recommendations</h4>
        {sourceBadge}
      </div>

      {loading && empty ? (
        <p className="text-muted text-sm">Yazg is generating recommendations from findings…</p>
      ) : error ? (
        <p className="text-danger text-sm">{error}</p>
      ) : empty ? (
        <p className="text-muted text-sm">No recommendations available yet.</p>
      ) : (
        <>
          {overview ? (
            <p className="wizard-results__recommendations-overview">{overview}</p>
          ) : null}
          <ul className="wizard-results__recommendation-list">
            {recommendations.map((item, index) => (
              <li
                key={`${item.title}-${item.priority}-${index}`}
                className="wizard-results__recommendation-item"
              >
                <div className="wizard-results__recommendation-row">
                  <Badge variant={recommendationPriorityVariant(item.priority)}>
                    {item.priority}
                  </Badge>
                  <span className="wizard-results__recommendation-name">{item.title}</span>
                </div>
                <p className="wizard-results__recommendation-description text-sm">
                  {item.description}
                </p>
                {renderActionButtons(item)}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
