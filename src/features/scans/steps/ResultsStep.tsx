import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Button } from "@/shared/components";
import {
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
} from "@/features/reports/reportDownloads";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import type { Severity } from "@/shared/types";

type ResultsStepProps = {
  projectId: string;
  scanId: string;
};

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];

function severityVariant(severity: Severity): "danger" | "warning" | "info" | "muted" {
  if (severity === "critical" || severity === "high") return "danger";
  if (severity === "medium") return "warning";
  if (severity === "low") return "info";
  return "muted";
}

export function ResultsStep({ projectId, scanId }: ResultsStepProps) {
  const { scans, findings, actions } = useAppStore();

  const [exporting, setExporting] = useState<ReportExportFormat | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);

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

  const statusCounts = useMemo(() => {
    const counts = { open: 0, confirmed: 0, false_positive: 0, fixed: 0 };
    for (const finding of scanFindings) {
      counts[finding.status] += 1;
    }
    return counts;
  }, [scanFindings]);

  const topFindings = useMemo(
    () =>
      [...scanFindings]
        .sort((a, b) => severityRank(a.severity) - severityRank(b.severity))
        .slice(0, 8),
    [scanFindings],
  );

  async function handleExport(format: ReportExportFormat) {
    setExporting(format);
    setExportError(null);
    setExportPath(null);
    try {
      const path = await generateAndExportScanReport(projectId, scanId, format);
      setExportPath(path);
      await actions.refresh();
    } catch (err) {
      setExportError(err instanceof Error ? err.message : "Report export failed");
    } finally {
      setExporting(null);
    }
  }

  const scanRunning = status.status === "running" || status.status === "paused";

  return (
    <div className="wizard-results">
      {scanRunning && (
        <div className="wizard-results__banner">
          <p>
            Scan still in progress ({status.progress_percent}% · {status.findings_count}{" "}
            finding{status.findings_count === 1 ? "" : "s"} so far). Results update automatically.
          </p>
          <Button variant="ghost" size="sm" onClick={() => void actions.refresh()}>
            Refresh data
          </Button>
        </div>
      )}

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Severity summary</h3>
        <div className="wizard-results__severity-grid">
          {SEVERITY_ORDER.map((severity) => (
            <div key={severity} className="wizard-results__severity-card">
              <Badge variant={severityVariant(severity)}>{severity}</Badge>
              <span className="wizard-results__severity-count">{severityCounts.get(severity) ?? 0}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Findings summary</h3>
        <div className="wizard-results__stats">
          <span>{scanFindings.length} total</span>
          <span>{statusCounts.open} open</span>
          <span>{statusCounts.confirmed} confirmed</span>
          <span>{statusCounts.false_positive} false positive</span>
          <span>{statusCounts.fixed} fixed</span>
        </div>

        {scanFindings.length === 0 ? (
          <p className="text-muted">
            {scanRunning
              ? "No findings recorded yet. Attack tests are still running."
              : "No findings were recorded for this scan."}
          </p>
        ) : (
          <ul className="wizard-results__finding-list">
            {topFindings.map((finding) => (
              <li key={finding.id} className="wizard-results__finding-row">
                <Badge variant={severityVariant(finding.severity)}>{finding.severity}</Badge>
                <span className="wizard-results__finding-title">{finding.title}</span>
                <span className="text-muted">{finding.category}</span>
              </li>
            ))}
          </ul>
        )}

        {scanFindings.length > topFindings.length && (
          <Link to={`/findings?scanId=${encodeURIComponent(scanId)}`} className="wizard-results__link">
            View all {scanFindings.length} findings →
          </Link>
        )}
      </section>

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Generate report</h3>
        <p className="text-muted">
          Export a report from SQLite for this scan. Reports include findings captured at generation
          time.
        </p>
        <div className="wizard-results__export-actions">
          {(["html", "pdf", "sarif"] as ReportExportFormat[]).map((format) => (
            <Button
              key={format}
              variant={format === "html" ? "primary" : "secondary"}
              disabled={scanFindings.length === 0 || exporting !== null}
              onClick={() => void handleExport(format)}
            >
              {exporting === format ? "Generating…" : `Download ${reportExportLabel(format)}`}
            </Button>
          ))}
        </div>
        {exportError && <p className="text-danger">{exportError}</p>}
        {exportPath && (
          <p className="wizard-results__export-path text-muted">Saved to {exportPath}</p>
        )}
      </section>
    </div>
  );
}

function severityRank(severity: Severity): number {
  const idx = SEVERITY_ORDER.indexOf(severity);
  return idx === -1 ? SEVERITY_ORDER.length : idx;
}
