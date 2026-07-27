import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";
import { Link, useNavigate } from "react-router-dom";

import { buildScanRetryUrl } from "@/features/scans/wizardState";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { Badge, Button, YazgBadge } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import {
  generateProjectSummary,
  type ProjectSummaryActionDto,
  type ProjectSummaryFailedScanDto,
  type ProjectSummaryResponse,
} from "@/shared/ipc/projectSummary";

type ProjectSummaryPanelProps = {
  projectId: string;
  /** Summary generation requires at least one target. */
  enabled?: boolean;
  /**
   * When posture inputs change (targets / scans / findings), remount reload
   * so the backend can regenerate if its fingerprint is stale.
   */
  revision?: string;
};

function isRetryAction(item: ProjectSummaryActionDto): boolean {
  return item.action.trim().toLowerCase() === "retry_scan";
}

function looksLikeRetryHighlight(text: string): boolean {
  const t = text.toLowerCase();
  return (
    t.includes("retry scan") ||
    t.includes("re-run") ||
    t.includes("rerun") ||
    (t.includes("failed") && t.includes("scan"))
  );
}

/** Replace endpoint URLs and scan IDs in LLM-generated text with links. */
function linkifyHighlight(
  text: string,
  failedScans: ProjectSummaryFailedScanDto[],
): ReactNode {
  if (failedScans.length === 0) return text;

  type Needle = {
    value: string;
    kind: "target" | "scan";
    targetId?: string | null;
    scanId: string;
  };

  const needles: Needle[] = [];
  for (const item of failedScans) {
    const url = item.target_url?.trim();
    if (url) {
      needles.push({
        value: url,
        kind: "target",
        targetId: item.target_id,
        scanId: item.scan_id,
      });
    }
    needles.push({
      value: item.scan_id,
      kind: "scan",
      targetId: item.target_id,
      scanId: item.scan_id,
    });
  }

  needles.sort((a, b) => b.value.length - a.value.length);

  type Seg =
    | { type: "text"; value: string }
    | { type: "link"; needle: Needle; value: string };

  let segments: Seg[] = [{ type: "text", value: text }];

  for (const needle of needles) {
    if (!needle.value) continue;
    const next: Seg[] = [];
    for (const seg of segments) {
      if (seg.type !== "text") {
        next.push(seg);
        continue;
      }
      let remaining = seg.value;
      while (remaining.length > 0) {
        const idx = remaining.indexOf(needle.value);
        if (idx < 0) {
          next.push({ type: "text", value: remaining });
          break;
        }
        if (idx > 0) {
          next.push({ type: "text", value: remaining.slice(0, idx) });
        }
        next.push({
          type: "link",
          needle,
          value: needle.value,
        });
        remaining = remaining.slice(idx + needle.value.length);
      }
    }
    segments = next;
  }

  return (
    <>
      {segments.map((seg, index) => {
        if (seg.type === "text") {
          return <Fragment key={`t-${index}`}>{seg.value}</Fragment>;
        }
        if (seg.needle.kind === "target" && seg.needle.targetId) {
          return (
            <Link
              key={`l-${index}`}
              className="project-summary__link"
              to={`/targets/${seg.needle.targetId}`}
            >
              {seg.value}
            </Link>
          );
        }
        if (seg.needle.kind === "scan") {
          return (
            <Link
              key={`l-${index}`}
              className="project-summary__link mono"
              to={`/scans/${seg.needle.scanId}`}
            >
              {seg.value}
            </Link>
          );
        }
        return <Fragment key={`l-${index}`}>{seg.value}</Fragment>;
      })}
    </>
  );
}

export function ProjectSummaryPanel({
  projectId,
  enabled = true,
  revision = "",
}: ProjectSummaryPanelProps) {
  const navigate = useNavigate();
  const [summary, setSummary] = useState<ProjectSummaryResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);
  const requestIdRef = useRef(0);

  const load = (force: boolean) => {
    if (!enabled) return;
    const requestId = ++requestIdRef.current;
    const fetchKey = `${projectId}:${revision}`;
    setLoading(true);
    setError(null);

    void generateProjectSummary(projectId, force)
      .then((response) => {
        if (requestId !== requestIdRef.current) return;
        fetchedRef.current = fetchKey;
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
    const fetchKey = `${projectId}:${revision}`;
    if (fetchedRef.current === fetchKey) return;

    // Debounce posture-driven reloads so live scan finding ticks coalesce.
    const delayMs = fetchedRef.current?.startsWith(`${projectId}:`) ? 1500 : 0;
    const timer = window.setTimeout(() => {
      if (fetchedRef.current === fetchKey) return;
      load(false);
    }, delayMs);

    return () => {
      window.clearTimeout(timer);
      requestIdRef.current += 1;
    };
  }, [projectId, enabled, revision]);

  const empty = !summary;
  const failedScans = summary?.failed_scans ?? [];
  const retryActions =
    summary?.actions?.filter(isRetryAction) ??
    failedScans.map((scan) => ({
      title: "Retry Scan",
      description: "",
      action: "retry_scan",
      scan_id: scan.scan_id,
      target_id: scan.target_id,
    }));
  const retryHighlightIndex =
    summary?.highlights.findIndex((h) => looksLikeRetryHighlight(h)) ?? -1;
  const sourceBadge =
    summary?.source === "ai" ? (
      <YazgBadge pulsing={loading} />
    ) : summary?.source === "fallback" ? (
      <Badge variant="muted">Rule-based guidance</Badge>
    ) : null;

  function handleRetryScan(scanId: string, targetId?: string | null) {
    navigate(buildScanRetryUrl(projectId, scanId, targetId));
  }

  function retryButtonLabel(action: ProjectSummaryActionDto): string {
    if (retryActions.length <= 1) return "Retry Scan";
    const match = failedScans.find((s) => s.scan_id === action.scan_id);
    const endpoint = match?.target_url?.trim() || match?.target_name?.trim();
    if (endpoint) {
      const short =
        endpoint.length > 42 ? `${endpoint.slice(0, 39)}…` : endpoint;
      return `Retry · ${short}`;
    }
    return `Retry · ${action.scan_id}`;
  }

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
              {summary.highlights.map((item, index) => {
                const showRetry =
                  retryActions.length > 0 &&
                  (retryHighlightIndex >= 0
                    ? index === retryHighlightIndex
                    : index === 0 && failedScans.length > 0);
                return (
                  <li key={`${index}-${item}`} className="scan-rec__item">
                    <span className="scan-rec__index" aria-hidden="true">
                      {String(index + 1).padStart(2, "0")}
                    </span>
                    <div className="scan-rec__body">
                      <p className="scan-rec__item-desc">
                        {failedScans.length > 0
                          ? linkifyHighlight(item, failedScans)
                          : item}
                      </p>
                      {showRetry ? (
                        <div className="scan-rec__actions project-summary__retry-actions">
                          {retryActions.map((action) => (
                            <Button
                              key={action.scan_id}
                              variant="primary"
                              size="sm"
                              title={action.description || undefined}
                              onClick={() =>
                                handleRetryScan(action.scan_id, action.target_id)
                              }
                            >
                              {retryButtonLabel(action)}
                            </Button>
                          ))}
                        </div>
                      ) : null}
                    </div>
                  </li>
                );
              })}
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
                {loading ? "Re-summarizing…" : "Re-summarize"}
              </span>
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
