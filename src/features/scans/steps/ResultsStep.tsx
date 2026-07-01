import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Button, RefreshButton } from "@/shared/components";
import {
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
} from "@/features/reports/reportDownloads";
import { FindingDetailPanel } from "@/features/findings/FindingDetailPanel";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import type { Severity } from "@/shared/types";

type ResultsStepProps = {
  projectId: string;
  scanId: string;
  onDone?: () => void;
};

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];

function severityVariant(severity: Severity): "danger" | "warning" | "info" | "muted" {
  if (severity === "critical" || severity === "high") return "danger";
  if (severity === "medium") return "warning";
  if (severity === "low") return "info";
  return "muted";
}

export function ResultsStep({ projectId, scanId, onDone }: ResultsStepProps) {
  const { scans, findings, actions, loading, error } = useAppStore();

  const [exporting, setExporting] = useState<ReportExportFormat | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [selectedFindingId, setSelectedFindingId] = useState<string | null>(null);

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

  const topFindings = useMemo(
    () =>
      [...scanFindings]
        .sort((a, b) => severityRank(a.severity) - severityRank(b.severity))
        .slice(0, 8),
    [scanFindings],
  );

  const selectedFinding = useMemo(
    () => scanFindings.find((finding) => finding.id === selectedFindingId) ?? null,
    [scanFindings, selectedFindingId],
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
        <h3 className="wizard-results__heading">Scan summary</h3>
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
        </dl>
      </section>

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
                <button
                  type="button"
                  className={`wizard-results__finding-button${selectedFindingId === finding.id ? " wizard-results__finding-button--selected" : ""}`}
                  onClick={() =>
                    setSelectedFindingId((current) =>
                      current === finding.id ? null : finding.id,
                    )
                  }
                >
                  <Badge variant={severityVariant(finding.severity)}>{finding.severity}</Badge>
                  <span className="wizard-results__finding-title">{finding.title}</span>
                  <span className="text-muted">{finding.category}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        {selectedFinding && (
          <div className="wizard-results__finding-detail">
            <FindingDetailPanel
              finding={selectedFinding}
              onClose={() => setSelectedFindingId(null)}
            />
          </div>
        )}
      </section>

      <section className="wizard-results__section">
        <h3 className="wizard-results__heading">Report actions</h3>
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

      <div className="wizard-results__footer-actions">
        <Link to={`/findings?scanId=${encodeURIComponent(scanId)}`}>
          <Button variant="secondary">View Findings</Button>
        </Link>
        <Link to="/scans">
          <Button variant="secondary">Open Scan Monitor</Button>
        </Link>
        {onDone && (
          <Button variant="primary" onClick={onDone}>
            Done
          </Button>
        )}
      </div>
    </div>
  );
}

function severityRank(severity: Severity): number {
  const idx = SEVERITY_ORDER.indexOf(severity);
  return idx === -1 ? SEVERITY_ORDER.length : idx;
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
