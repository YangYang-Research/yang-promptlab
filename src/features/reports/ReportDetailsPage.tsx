import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { parseFindingEvidence, formatHttpRequest, formatHttpResponse } from "@/features/findings/findingEvidence";
import {
  Badge,
  Button,
  Card,
  EmptyState,
  FindingStatusBadge,
  PageHeader,
  PageLoadingSkeleton,
  SeverityBadge,
  StatCard,
  StatusBadge,
} from "@/shared/components";
import { useToast } from "@/shared/notifications";
import { severityCounts } from "@/shared/stats";
import type { Finding, Severity } from "@/shared/types";

import {
  exportStoredReport,
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
} from "./reportDownloads";
import { recommendationFor } from "./findingRecommendation";

const EXPORT_FORMATS: ReportExportFormat[] = ["html", "pdf", "sarif", "csv"];
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

export function ReportDetailsPage() {
  const { reportId = "" } = useParams();
  const { reports, scans, findings, projects, targets, loading, actions } = useAppStore();
  const { notify } = useToast();
  const [busyFormat, setBusyFormat] = useState<ReportExportFormat | null>(null);

  const report = reports.find((item) => item.id === reportId);
  const scan = scans.find((item) => item.id === report?.scanId);
  const project = projects.find((item) => item.id === report?.projectId);
  const target = targets.find((item) => item.id === scan?.targetId);
  const reportFindings = useMemo(
    () => (report?.scanId ? findings.filter((finding) => finding.scanId === report.scanId) : []),
    [findings, report?.scanId],
  );
  const counts = useMemo(() => severityCounts(reportFindings), [reportFindings]);
  const riskScore = useMemo(() => computeRiskScore(reportFindings), [reportFindings]);

  async function handleExport(format: ReportExportFormat) {
    if (!report?.scanId) return;
    setBusyFormat(format);
    try {
      const dest =
        format === "html" && report.format === "html"
          ? await exportStoredReport(report.id)
          : await generateAndExportScanReport(report.projectId, report.scanId, format);
      await actions.refresh();
      notify(`${reportExportLabel(format)} report saved to ${dest}`, "success");
    } catch (err) {
      notify(err instanceof Error ? err.message : "Report export failed", "error");
    } finally {
      setBusyFormat(null);
    }
  }

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
          <div className="report-viewer__exports" aria-label="Export report">
            {EXPORT_FORMATS.map((format) => (
              <Button
                key={format}
                size="sm"
                variant={format === "html" ? "primary" : "secondary"}
                disabled={!report.scanId || busyFormat !== null}
                onClick={() => void handleExport(format)}
              >
                {busyFormat === format ? "…" : reportExportLabel(format)}
              </Button>
            ))}
          </div>
        }
      />

      <Card className="report-native__identity">
        <div>
          <span className="report-native__eyebrow">Technical security report</span>
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
        <Card className="detail-section">
          <h2 className="detail-section__title">Executive summary</h2>
          <p>
            The scan identified <strong>{reportFindings.length}</strong> findings across the
            assessed target. The current risk level is <strong>{riskLabel(riskScore)}</strong>,
            including <strong>{counts.critical}</strong> critical and{" "}
            <strong>{counts.high}</strong> high-severity findings.
          </p>
          <p className="text-muted">
            Prioritize confirmed critical and high findings, apply mitigations, then re-run the
            scan to verify remediation.
          </p>
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Severity distribution</h2>
          <div className="report-native__severity-list">
            {SEVERITIES.map((severity) => (
              <div key={severity} className="report-native__severity-row">
                <SeverityBadge severity={severity} />
                <strong>{counts[severity]}</strong>
              </div>
            ))}
          </div>
        </Card>
      </section>

      <section className="reports-section" aria-label="Detailed findings">
        <div className="reports-section__header">
          <div>
            <h2 className="reports-section__title">Detailed findings</h2>
            <span className="text-muted text-sm">{reportFindings.length} findings</span>
          </div>
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
            {reportFindings.map((finding) => {
              const evidence = parseFindingEvidence(finding);
              const recommendation = recommendationFor(finding.category);
              const confidencePct = Math.round(
                finding.confidence > 1 ? finding.confidence : finding.confidence * 100,
              );
              const endpoint =
                evidence.requestUrl ??
                (finding.targetUrl || null);

              return (
                <Card key={finding.id} className="report-native__finding">
                  <div className="row-actions">
                    <SeverityBadge severity={finding.severity} />
                    <FindingStatusBadge status={finding.status} />
                    <Badge variant="muted">{finding.category.replace(/_/g, " ")}</Badge>
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
                      <span className="report-native__evidence-label">Recommendation</span>
                      <p className="report-native__recommendation">
                        <strong>{recommendation.title}</strong>
                        <span>{recommendation.description}</span>
                      </p>
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
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
