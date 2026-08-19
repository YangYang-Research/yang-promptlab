import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Card, ConfidenceMeter, DataTable, FindingStatusBadge, Pagination, RefreshButton, SeverityBadge } from "@/shared/components";
import { IconCheck } from "@/shared/components/Icons";
import { resolveAttackGraphStates } from "@/features/scans/attackGraphProgress";
import { type AttackCategoryId } from "@/features/scans/attackProfiles";
import { categoryLabel } from "@/features/scans/categoryLabel";
import { buildSeverityBreakdown } from "@/features/scans/resultsSeverityBreakdown";
import { ScanRecommendationsPanel } from "@/features/scans/ScanRecommendationsPanel";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import type { Finding, Severity } from "@/shared/types";

type ResultsStepProps = {
  scanId: string;
  attackCategories?: AttackCategoryId[];
  onNewScan?: () => void;
  onRetryScan?: () => void;
  onChangeAttackPlan?: () => void;
};

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];
const FINDINGS_SUMMARY_PAGE_SIZE = 10;

type FindingSummaryRow = Finding & { no: number };

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
  onNewScan,
  onRetryScan,
  onChangeAttackPlan,
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

  const summaryRows = useMemo<FindingSummaryRow[]>(
    () => scanFindings.map((finding, index) => ({ ...finding, no: index + 1 })),
    [scanFindings],
  );
  const {
    page: findingsPage,
    setPage: setFindingsPage,
    pagination: findingsPagination,
  } = usePaginatedList(summaryRows, FINDINGS_SUMMARY_PAGE_SIZE);
  const summaryColumns = useMemo(
    () => [
      {
        key: "no",
        header: "No",
        width: "56px",
        render: (row: FindingSummaryRow) => row.no,
      },
      {
        key: "category",
        header: "Category",
        width: "160px",
        render: (row: FindingSummaryRow) => categoryLabel(row.category),
      },
      {
        key: "finding",
        header: "Finding",
        render: (row: FindingSummaryRow) => (
          <button
            type="button"
            className="link wizard-results__finding-link"
            onClick={() => navigate(`/findings/${row.id}`)}
          >
            {row.title}
          </button>
        ),
      },
      {
        key: "severity",
        header: "Severity",
        width: "110px",
        render: (row: FindingSummaryRow) => <SeverityBadge severity={row.severity} />,
      },
      {
        key: "confidence",
        header: "Confidence",
        width: "140px",
        render: (row: FindingSummaryRow) => <ConfidenceMeter confidence={row.confidence} />,
      },
      {
        key: "status",
        header: "Status",
        width: "120px",
        render: (row: FindingSummaryRow) => <FindingStatusBadge status={row.status} />,
      },
    ],
    [navigate],
  );

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
        <h3 className="wizard-results__heading">Attack Summary</h3>
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
      </section>

      <section className="wizard-results__section" aria-label="Recommendations">
        <ScanRecommendationsPanel
          scanId={scanId}
          attackCategories={attackCategories as string[]}
          projectId={scan?.projectId}
          targetId={scan?.targetId}
          onNewScan={onNewScan}
          onRetryScan={onRetryScan}
          onChangeAttackPlan={onChangeAttackPlan}
          revision={
            status.status === "running" || status.status === "paused"
              ? status.status
              : `${status.status}|${scanFindings
                  .map((f) => `${f.id}:${f.severity}:${f.title}`)
                  .sort()
                  .join(",")}`
          }
        />
      </section>

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Severity Summary</h3>
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
                <SeverityBadge severity={severity} />
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

      <section className="wizard-results__section" aria-label="Findings Summary">
        <h3 className="wizard-results__heading">Findings Summary</h3>
        <Card padding="none">
          <DataTable
            columns={summaryColumns}
            rows={findingsPagination.items}
            keyField="id"
            emptyMessage={
              scanRunning
                ? "No findings recorded yet. Attack tests are still running."
                : "No findings were recorded for this scan."
            }
          />
        </Card>
        {summaryRows.length > 0 ? (
          <Pagination
            page={findingsPage}
            totalItems={findingsPagination.totalItems}
            rangeStart={findingsPagination.rangeStart}
            rangeEnd={findingsPagination.rangeEnd}
            totalPages={findingsPagination.totalPages}
            onPageChange={setFindingsPage}
          />
        ) : null}
      </section>
    </div>
  );
}
