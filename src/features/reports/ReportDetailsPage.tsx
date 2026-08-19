import { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { parseFindingEvidence, formatHttpRequest, formatHttpResponse } from "@/features/findings/findingEvidence";
import { FindingRecommendationsPanel } from "@/features/findings/FindingRecommendationsPanel";
import { DoughnutChart } from "@/features/dashboard/SeverityDoughnutChart";
import { categoryLabel } from "@/features/scans/categoryLabel";
import { buildFindingsByCategory } from "@/features/scans/FindingsByCategoryChart";
import { parseAttackPlaybook } from "@/features/scans/scanPlaybook";
import { ReportExportDropdown } from "@/features/scans/components/ReportExportDropdown";
import { ScanRecommendationsPanel } from "@/features/scans/ScanRecommendationsPanel";
import {
  AttackCategoryBadge,
  Badge,
  Card,
  ConfidenceMeter,
  DataTable,
  EmptyState,
  FindingStatusBadge,
  PageHeader,
  PageLoadingSkeleton,
  Pagination,
  RefreshButton,
  SeverityBadge,
  StatCard,
  StatusBadge,
} from "@/shared/components";
import { getScan, type ScanDetailDto } from "@/shared/ipc";
import { severityCounts } from "@/shared/stats";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import type { Finding, Severity } from "@/shared/types";

const FINDINGS_SUMMARY_PAGE_SIZE = 10;
const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];
const RISK_WEIGHT: Record<Severity, number> = {
  critical: 16,
  high: 8,
  medium: 4,
  low: 2,
  info: 1,
};

function computeRiskScore(findings: Finding[]): number {
  if (findings.length === 0) return 0;
  const risk = findings.reduce((sum, finding) => sum + RISK_WEIGHT[finding.severity], 0);
  return Math.round(Math.min(100, (risk / (findings.length * RISK_WEIGHT.critical)) * 100));
}

function riskLabel(score: number): string {
  if (score >= 75) return "Critical";
  if (score >= 50) return "High";
  if (score >= 25) return "Medium";
  if (score > 0) return "Low";
  return "No detected risk";
}

function confidencePercent(confidence: number): number {
  return Math.round(confidence > 1 ? confidence : confidence * 100);
}

function scrollToFindingDetail(findingId: string) {
  const el = document.getElementById(`finding-${findingId}`);
  if (!el) return;
  el.scrollIntoView({ behavior: "smooth", block: "start" });
  // Keep deep-linkable hash without relying on window scroll for nested overflow.
  window.history.replaceState(null, "", `#finding-${findingId}`);
}

type FindingSummaryRow = Finding & { no: number };

export function ReportDetailsPage() {
  const { reportId = "" } = useParams();
  const { reports, scans, findings, projects, targets, loading, error, actions } = useAppStore();
  const [scanDetail, setScanDetail] = useState<ScanDetailDto | null>(null);

  const report = reports.find((item) => item.id === reportId);
  const scan = scans.find((item) => item.id === report?.scanId);
  const project = projects.find((item) => item.id === report?.projectId);
  const target = targets.find((item) => item.id === scan?.targetId);
  const reportFindings = useMemo(
    () => (report?.scanId ? findings.filter((finding) => finding.scanId === report.scanId) : []),
    [findings, report?.scanId],
  );
  const playbook = useMemo(
    () => parseAttackPlaybook(scanDetail?.playbook),
    [scanDetail?.playbook],
  );
  const recommendationsRevision = useMemo(
    () =>
      `${scan?.status ?? "unknown"}|${reportFindings
        .map((finding) => `${finding.id}:${finding.severity}:${finding.title}`)
        .sort()
        .join(",")}`,
    [scan?.status, reportFindings],
  );

  useEffect(() => {
    const scanId = report?.scanId;
    if (!scanId) {
      setScanDetail(null);
      return;
    }
    let cancelled = false;
    void getScan(scanId)
      .then((detail) => {
        if (!cancelled) setScanDetail(detail);
      })
      .catch(() => {
        if (!cancelled) setScanDetail(null);
      });
    return () => {
      cancelled = true;
    };
  }, [report?.scanId]);

  const counts = useMemo(() => severityCounts(reportFindings), [reportFindings]);
  const maxSeverityCount = Math.max(...SEVERITIES.map((severity) => counts[severity]), 1);
  const findingsByCategory = useMemo(
    () => buildFindingsByCategory(reportFindings),
    [reportFindings],
  );
  const riskScore = useMemo(() => computeRiskScore(reportFindings), [reportFindings]);
  const summaryRows = useMemo<FindingSummaryRow[]>(
    () => reportFindings.map((finding, index) => ({ ...finding, no: index + 1 })),
    [reportFindings],
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
          <a
            className="link"
            href={`#finding-${row.id}`}
            onClick={(event) => {
              event.preventDefault();
              scrollToFindingDetail(row.id);
            }}
          >
            {row.title}
          </a>
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
    [],
  );

  if (!report && !loading) {
    return (
      <div className="page">
        <PageHeader title="Report details" backTo="/reports" />
        <EmptyState
          title="Report not found"
          description="This report may have been deleted or is no longer available."
        />
      </div>
    );
  }

  if (!report) {
    return (
      <div className="page">
        <PageHeader title="Report details" backTo="/reports" />
        <PageLoadingSkeleton />
      </div>
    );
  }

  return (
    <div className="page report-native">
      <PageHeader
        backTo="/reports"
        backOnly
        title="Report details"
        actions={
          <div className="page-actions">
            <RefreshButton
              size="sm"
              loading={loading}
              error={error}
              onClick={() => void actions.refresh()}
            />
            {report.scanId ? (
              <ReportExportDropdown
                projectId={report.projectId}
                scanId={report.scanId}
                findingsCount={reportFindings.length}
                requireFindings={false}
                onExported={() => void actions.refresh()}
              />
            ) : null}
          </div>
        }
      />

      <Card className="report-native__identity">
        <div>
          <span className="report-native__eyebrow">PromptLab - Security Scan Report</span>
          <h2>{report.scanName}</h2>
          <p className="text-muted">
            Generated {new Date(report.createdAt).toLocaleString()}
          </p>
        </div>
        <StatusBadge status={report.status} />
        <dl className="report-native__metadata">
          <div>
            <dt>Report ID</dt>
            <dd className="mono text-sm">{report.id}</dd>
          </div>
          <div>
            <dt>Project</dt>
            <dd>{project?.name ?? report.projectName}</dd>
          </div>
          <div>
            <dt>Scan ID</dt>
            <dd>
              {report.scanId ? (
                <Link className="link mono text-sm" to={`/scans/${report.scanId}`}>
                  {report.scanId}
                </Link>
              ) : (
                "—"
              )}
            </dd>
          </div>
          <div>
            <dt>Target</dt>
            <dd>{target?.name ?? "—"}</dd>
          </div>
        </dl>
      </Card>

      <section className="report-native__stats" aria-label="Report summary">
        <StatCard
          label="Risk score"
          value={`${riskScore}/100`}
          hint={riskLabel(riskScore)}
          accent={riskScore >= 50 ? "critical" : riskScore >= 25 ? "warning" : "success"}
        />
        <StatCard
          label="Total findings"
          value={reportFindings.length}
          hint={`${counts.critical + counts.high} critical or high`}
        />
        <StatCard
          label="Confirmed"
          value={reportFindings.filter((finding) => finding.status === "confirmed").length}
          hint="Validated vulnerabilities"
        />
        <StatCard
          label="Open"
          value={reportFindings.filter((finding) => finding.status === "open").length}
          hint="Awaiting triage"
        />
      </section>

      <section className="report-native__grid">
        <div className="report-native__charts">
          <Card className="detail-section">
            <h2 className="detail-section__title">Severity Distribution</h2>
            <div className="severity-chart">
              {SEVERITIES.map((severity) => (
                <div key={severity} className="severity-chart__row">
                  <SeverityBadge severity={severity} />
                  <div className="severity-chart__bar-track">
                    <div
                      className={`severity-chart__bar severity-chart__bar--${severity}`}
                      style={{ width: `${(counts[severity] / maxSeverityCount) * 100}%` }}
                    />
                  </div>
                  <span className="severity-chart__count">{counts[severity]}</span>
                </div>
              ))}
            </div>
          </Card>

          <Card className="detail-section">
            <h2 className="detail-section__title">Findings by Category</h2>
            <DoughnutChart
              data={findingsByCategory}
              size={176}
              emptyMessage="No findings by attack category yet."
              ariaLabel="Findings by attack category"
            />
          </Card>
        </div>

        <Card className="detail-section report-native__executive">
          {report.scanId ? (
            <ScanRecommendationsPanel
              title="Executive Summary"
              scanId={report.scanId}
              attackCategories={playbook?.categories ?? []}
              enabled={!loading && Boolean(report.scanId)}
              showReRecommend={false}
              projectId={report.projectId}
              targetId={scan?.targetId}
              revision={recommendationsRevision}
            />
          ) : (
            <>
              <h2 className="detail-section__title">Executive Summary</h2>
              <p className="text-muted">No linked scan for this report.</p>
            </>
          )}
        </Card>
      </section>

      <section className="reports-section" aria-label="Findings summary">
        <div className="reports-section__header">
          <h2 className="reports-section__title">Findings Summary</h2>
        </div>
        <Card padding="none">
          <DataTable
            columns={summaryColumns}
            rows={findingsPagination.items}
            keyField="id"
            emptyMessage="No findings"
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

      <section className="reports-section" aria-label="Detailed Findings">
        <div className="reports-section__header">
          <h2 className="reports-section__title">Detailed Findings</h2>
        </div>

        {reportFindings.length === 0 ? (
          <Card>
            <EmptyState
              title="No findings"
              description="This scan completed without reportable findings."
            />
          </Card>
        ) : (
          <div className="report-native__findings">
            {reportFindings.map((finding, index) => {
              const evidence = parseFindingEvidence(finding);
              const confidencePct = confidencePercent(finding.confidence);
              const endpoint =
                evidence.requestUrl ??
                (finding.targetUrl || null);

              return (
                <div
                  key={finding.id}
                  id={`finding-${finding.id}`}
                  className="report-native__finding-anchor"
                >
                <Card className="report-native__finding">
                  <div className="row-actions">
                    <Badge variant="muted">#{index + 1}</Badge>
                    <SeverityBadge severity={finding.severity} />
                    <FindingStatusBadge status={finding.status} />
                    <AttackCategoryBadge category={finding.category} />
                  </div>

                  <h3>
                    <Link className="link" to={`/findings/${finding.id}`}>
                      {finding.title}
                    </Link>
                  </h3>

                  <p className="report-native__confidence text-muted text-sm">
                    Confidence {confidencePct}%
                  </p>
                  <p>{finding.description || "No description provided."}</p>

                  <div className="report-native__meta">
                    <div>
                      <span className="report-native__evidence-label">Endpoint</span>
                      <code>{endpoint ?? "—"}</code>
                    </div>
                    <div>
                      <span className="report-native__evidence-label">Payload</span>
                      <pre>{evidence.payload?.trim() || "—"}</pre>
                    </div>
                    <div>
                      <FindingRecommendationsPanel
                        finding={finding}
                        variant="embedded"
                        showReRecommend={false}
                        queueOrder={index}
                      />
                    </div>
                  </div>

                  <div className="report-native__traffic">
                    <div>
                      <span className="report-native__evidence-label">Request</span>
                      <pre>{formatHttpRequest(evidence)}</pre>
                    </div>
                    <div>
                      <span className="report-native__evidence-label">Response</span>
                      <pre>{formatHttpResponse(evidence)}</pre>
                    </div>
                  </div>
                </Card>
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
