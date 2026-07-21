import { useEffect, useRef, useState } from "react";

import { Badge, Button, YazgBadge } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import {
  generateProjectSummary,
  type ProjectSummaryResponse,
} from "@/shared/ipc/projectSummary";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";

type ProjectSummaryPanelProps = {
  projectId: string;
  /** Summary generation requires at least one target. */
  enabled?: boolean;
};

export function ProjectSummaryPanel({
  projectId,
  enabled = true,
}: ProjectSummaryPanelProps) {
  const [summary, setSummary] = useState<ProjectSummaryResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);
  const requestIdRef = useRef(0);

  const load = (force: boolean) => {
    if (!enabled) return;
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);

    void generateProjectSummary(projectId, force)
      .then((response) => {
        if (requestId !== requestIdRef.current) return;
        fetchedRef.current = projectId;
        setSummary(response);
      })
      .catch((err) => {
        if (requestId !== requestIdRef.current) return;
        if (!force) fetchedRef.current = null;
        setError(err instanceof Error ? err.message : "Failed to load project summary");
      })
      .finally(() => {
        if (requestId === requestIdRef.current) setLoading(false);
      });
  };

  useEffect(() => {
    if (!enabled) {
      requestIdRef.current += 1;
      fetchedRef.current = null;
      setSummary(null);
      setLoading(false);
      setError(null);
      return;
    }
    if (!projectId) return;
    if (fetchedRef.current === projectId) return;
    load(false);
    return () => {
      requestIdRef.current += 1;
    };
  }, [projectId, enabled]);

  const empty = !summary;
  const sourceBadge =
    summary?.source === "ai" ? (
      <YazgBadge />
    ) : summary?.source === "fallback" ? (
      <Badge variant="muted">Rule-based guidance</Badge>
    ) : null;

  return (
    <div
      className="scan-rec"
      data-state={
        !enabled
          ? "empty"
          : loading && empty
            ? "loading"
            : error
              ? "error"
              : empty
                ? "empty"
                : "ready"
      }
    >
      <header className="scan-rec__header">
        <h2 className="scan-rec__title">Summary</h2>
        {enabled ? sourceBadge : null}
      </header>

      {!enabled ? (
        <p className="scan-rec__status text-muted text-sm">
          Add at least one target to generate a project summary.
        </p>
      ) : loading && empty ? (
        <div className="scan-rec__loading" aria-busy="true" aria-live="polite">
          <div className="scan-rec__skeleton scan-rec__skeleton--lead" />
          <div className="scan-rec__skeleton" />
          <div className="scan-rec__skeleton scan-rec__skeleton--short" />
          <p className="scan-rec__status text-muted text-sm">
            Yazg is summarizing this project…
          </p>
        </div>
      ) : error && empty ? (
        <p className="scan-rec__status text-danger text-sm">{error}</p>
      ) : !summary ? (
        <p className="scan-rec__status text-muted text-sm">No summary available yet.</p>
      ) : (
        <>
          {error ? <p className="scan-rec__status text-danger text-sm">{error}</p> : null}
          <p className="scan-rec__overview">{summary.overview}</p>

          {summary.highlights.length > 0 ? (
            <ol className="scan-rec__list">
              {summary.highlights.map((item, index) => (
                <li key={`${index}-${item}`} className="scan-rec__item">
                  <span className="scan-rec__index" aria-hidden="true">
                    {String(index + 1).padStart(2, "0")}
                  </span>
                  <div className="scan-rec__body">
                    <p className="scan-rec__item-desc">{item}</p>
                  </div>
                </li>
              ))}
            </ol>
          ) : null}

          <div className="project-summary__footer">
            {summary.generated_at ? (
              <p className="project-summary__generated">
                Generated {formatTimestamp(summary.generated_at)}
              </p>
            ) : (
              <span />
            )}
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
                {loading ? "Summarizing…" : "Summary"}
              </span>
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
