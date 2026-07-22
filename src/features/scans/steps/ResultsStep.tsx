import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, RefreshButton, YazgBadge } from "@/shared/components";
import { IconCheck } from "@/shared/components/Icons";
import { resolveAttackGraphStates } from "@/features/scans/attackGraphProgress";
import { getCategory, type AttackCategoryId } from "@/features/scans/attackProfiles";
import { buildSeverityBreakdown } from "@/features/scans/resultsSeverityBreakdown";
import { ScanRecommendationsPanel } from "@/features/scans/ScanRecommendationsPanel";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import type { Finding, Severity } from "@/shared/types";

type ResultsStepProps = {
  scanId: string;
  attackCategories?: AttackCategoryId[];
  onRetryScan?: () => void;
  onStartAttack?: () => void;
};

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];

function severityVariant(severity: Severity): "danger" | "warning" | "info" | "muted" {
  if (severity === "critical" || severity === "high") return "danger";
  if (severity === "medium") return "warning";
  if (severity === "low") return "info";
  return "muted";
}

function severityRank(severity: Severity): number {
  const idx = SEVERITY_ORDER.indexOf(severity);
  return idx === -1 ? SEVERITY_ORDER.length : idx;
}

function categoryLabel(categoryId: string): string {
  try {
    return getCategory(categoryId as AttackCategoryId).label;
  } catch {
    return categoryId.replace(/_/g, " ");
  }
}

function severityVariantForStatus(
  status: string,
): "success" | "warning" | "danger" | "info" | "muted" {
  if (status === "completed") return "success";
  if (status === "running") return "info";
  if (status === "paused") return "warning";
  if (status === "failed" || status === "stopped") return "danger";
  return "muted";
}

export function ResultsStep({
  scanId,
  attackCategories = [],
  onRetryScan,
  onStartAttack,
}: ResultsStepProps) {
  const { scans, findings, actions, loading, error } = useAppStore();
  const navigate = useNavigate();
  const [selectedSeverity, setSelectedSeverity] = useState<Severity | null>(null);

  const scan = scans.find((s) => s.id === scanId);
  const scanFindings = useMemo(
    () => findings.filter((f) => f.scanId === scanId),
    [findings, scanId],
  );

  const statuses = useScanStatuses([scanId], true);
  const live = statuses.get(scanId);
  const status = mergeScanStatus(scanId, scan?.status ?? "pending", live, scanFindings.length);

  const severityCounts = useMemo(() => {
    const counts = new Map<Severity, number>();
    for (const severity of SEVERITY_ORDER) counts.set(severity, 0);
    for (const finding of scanFindings) {
      counts.set(finding.severity, (counts.get(finding.severity) ?? 0) + 1);
    }
    return counts;
  }, [scanFindings]);

  const selectedSeverityBreakdown = useMemo(() => {
    if (!selectedSeverity) return [];
    return buildSeverityBreakdown(scanFindings, selectedSeverity, categoryLabel);
  }, [scanFindings, selectedSeverity]);

  const findingsByCategory = useMemo(() => {
    const groups = new Map<string, Finding[]>();
    const sorted = [...scanFindings].sort(
      (a, b) => severityRank(a.severity) - severityRank(b.severity),
    );
    for (const finding of sorted) {
      const bucket = groups.get(finding.category) ?? [];
      bucket.push(finding);
      groups.set(finding.category, bucket);
    }
    return [...groups.entries()].sort((a, b) => categoryLabel(a[0]).localeCompare(categoryLabel(b[0])));
  }, [scanFindings]);

  const categoryStates = useMemo(
    () => resolveAttackGraphStates(attackCategories, status),
    [attackCategories, status],
  );

  const scanRunning = status.status === "running" || status.status === "paused";

  return (
    <div className="wizard-results">
      {scanRunning && (
        <div className="wizard-results__banner">
          <p>
            Scan still in progress ({status.progress_percent}% · {status.findings_count}{" "}
            finding{status.findings_count === 1 ? "" : "s"} so far). Results update automatically.
          </p>
          <RefreshButton
            size="sm"
            ariaLabel="Refresh data"
            loading={loading}
            error={error}
            onClick={() => void actions.refresh()}
          />
        </div>
      )}

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Attack summary</h3>
        <dl className="wizard-results__summary-grid">
          <div>
            <dt>Status</dt>
            <dd>
              <Badge variant={severityVariantForStatus(status.status)}>{status.status}</Badge>
            </dd>
          </div>
          <div>
            <dt>Progress</dt>
            <dd>{status.progress_percent}%</dd>
          </div>
          <div>
            <dt>Findings</dt>
            <dd>{scanFindings.length}</dd>
          </div>
          {status.categories_completed !== undefined && attackCategories.length > 0 && (
            <div>
              <dt>Categories run</dt>
              <dd>
                {status.categories_completed}/{attackCategories.length}
              </dd>
            </div>
          )}
        </dl>
        {attackCategories.length > 0 && (
          <div className="wizard-results__attack-categories">
            <p className="wizard-results__attack-categories-label text-sm text-muted">Attack categories</p>
            <ul className="wizard-results__attack-category-list">
              {attackCategories.map((category) => {
                const state = categoryStates.get(category) ?? "pending";
                const isDone = state === "done";
                return (
                  <li key={category} className="wizard-results__attack-category-item">
                    <span className="wizard-results__attack-category-mark" aria-hidden="true">
                      {isDone ? <IconCheck className="wizard-results__attack-category-check" /> : null}
                    </span>
                    <span>{categoryLabel(category)}</span>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
        <ScanRecommendationsPanel
          scanId={scanId}
          attackCategories={attackCategories as string[]}
          variant="wizard"
          projectId={scan?.projectId}
          targetId={scan?.targetId}
          onRetryScan={onRetryScan}
          onStartAttack={onStartAttack}
        />
      </section>

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Severity summary</h3>
        <div className="wizard-results__severity-grid">
          {SEVERITY_ORDER.map((severity) => {
            const count = severityCounts.get(severity) ?? 0;
            const isSelected = selectedSeverity === severity;
            return (
              <button
                key={severity}
                type="button"
                className={`wizard-results__severity-card wizard-results__severity-card--button${isSelected ? " wizard-results__severity-card--selected" : ""}`}
                disabled={count === 0}
                aria-pressed={isSelected}
                onClick={() =>
                  setSelectedSeverity((current) => (current === severity ? null : severity))
                }
              >
                <Badge variant={severityVariant(severity)}>{severity}</Badge>
                <span className="wizard-results__severity-count">{count}</span>
              </button>
            );
          })}
        </div>
        {selectedSeverity && (
          <div className="wizard-results__severity-breakdown">
            <p className="wizard-results__severity-breakdown-title text-sm">
              Affected categories · <strong>{selectedSeverity}</strong>
            </p>
            {selectedSeverityBreakdown.length === 0 ? (
              <p className="text-muted text-sm">No findings at this severity.</p>
            ) : (
              <ul className="wizard-results__severity-category-list">
                {selectedSeverityBreakdown.map((group) => (
                  <li key={group.categoryId} className="wizard-results__severity-category-group">
                    <div className="wizard-results__severity-category-row">
                      <span className="wizard-results__severity-category-name">{group.categoryLabel}</span>
                      <span className="text-muted text-sm">
                        {group.totalCount} finding{group.totalCount === 1 ? "" : "s"}
                      </span>
                    </div>
                    <ul className="wizard-results__severity-subcategory-list">
                      {group.subcategories.map((subcategory) => (
                        <li
                          key={subcategory.label}
                          className="wizard-results__severity-subcategory-row"
                        >
                          <span>{subcategory.label}</span>
                          <span className="text-muted text-sm">
                            {subcategory.count} finding{subcategory.count === 1 ? "" : "s"}
                          </span>
                        </li>
                      ))}
                    </ul>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </section>

      <section className="wizard-results__section">
        <div className="wizard-results__heading-row">
          <h3 className="wizard-results__heading">Findings summary</h3>
          {scanFindings.length > 0 ? <YazgBadge /> : null}
        </div>
        {scanFindings.length === 0 ? (
          <p className="text-muted">
            {scanRunning
              ? "No findings recorded yet. Attack tests are still running."
              : "No findings were recorded for this scan."}
          </p>
        ) : (
          <div className="wizard-results__category-groups">
            {findingsByCategory.map(([category, categoryFindings]) => (
              <div key={category} className="wizard-results__category-group">
                <div className="wizard-results__category-header">
                  <h4 className="wizard-results__category-title">{categoryLabel(category)}</h4>
                  <span className="text-muted text-sm">{categoryFindings.length} finding{categoryFindings.length === 1 ? "" : "s"}</span>
                </div>
                <ul className="wizard-results__finding-list">
                  {categoryFindings.map((finding) => (
                    <li key={finding.id} className="wizard-results__finding-row">
                      <button
                        type="button"
                        className="wizard-results__finding-button"
                        onClick={() => navigate(`/findings/${finding.id}`)}
                      >
                        <Badge variant={severityVariant(finding.severity)}>{finding.severity}</Badge>
                        <span className="wizard-results__finding-title">{finding.title}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
