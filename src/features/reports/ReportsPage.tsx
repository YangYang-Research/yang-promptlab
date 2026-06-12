import { useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  StatusBadge,
} from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { Report } from "@/shared/types";

import {
  buildScanExportRows,
  exportStoredReport,
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
  type ScanExportRow,
} from "./reportDownloads";

const EXPORT_FORMATS: ReportExportFormat[] = ["html", "pdf", "sarif"];

export function ReportsPage() {
  const { reports, scans, findings, projects, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const exportRows = useMemo(
    () =>
      buildScanExportRows({
        scans,
        findings,
        reports,
        projects,
      }),
    [scans, findings, reports, projects],
  );

  const sortedReports = useMemo(
    () => [...reports].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [reports],
  );

  async function runExport(
    key: string,
    action: () => Promise<string>,
    successLabel: string,
  ) {
    setBusyKey(key);
    try {
      const dest = await action();
      await actions.refresh();
      notify(`${successLabel} saved to ${dest}`, "success");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Report export failed";
      notify(message, "error");
    } finally {
      setBusyKey(null);
    }
  }

  function handleScanExport(row: ScanExportRow, format: ReportExportFormat) {
    const key = `${row.scanId}:${format}`;
    void runExport(
      key,
      () => generateAndExportScanReport(row.projectId, row.scanId, format),
      `${reportExportLabel(format)} report`,
    );
  }

  function handleStoredExport(report: Report) {
    void runExport(
      report.id,
      () => exportStoredReport(report.id),
      `${report.format.toUpperCase()} report`,
    );
  }

  const exportColumns = [
    {
      key: "project",
      header: "Project",
      width: "150px",
      render: (row: ScanExportRow) => row.projectName,
    },
    {
      key: "scan",
      header: "Scan",
      render: (row: ScanExportRow) => (
        <div>
          <strong>{row.scanName}</strong>
          <div className="text-muted text-sm mono">{row.scanId}</div>
        </div>
      ),
    },
    {
      key: "findings",
      header: "Findings",
      width: "90px",
      render: (row: ScanExportRow) => row.findingCount,
    },
    {
      key: "generated",
      header: "Last generated",
      width: "170px",
      render: (row: ScanExportRow) =>
        row.lastGeneratedAt ? new Date(row.lastGeneratedAt).toLocaleString() : "—",
    },
    {
      key: "actions",
      header: "Export",
      width: "320px",
      render: (row: ScanExportRow) => (
        <span className="row-actions" onClick={(e) => e.stopPropagation()}>
          {EXPORT_FORMATS.map((format) => {
            const key = `${row.scanId}:${format}`;
            return (
              <Button
                key={format}
                size="sm"
                variant={format === "html" ? "primary" : "secondary"}
                disabled={busyKey === key}
                onClick={() => handleScanExport(row, format)}
              >
                {busyKey === key ? "…" : reportExportLabel(format)}
              </Button>
            );
          })}
        </span>
      ),
    },
  ];

  const archiveColumns = [
    {
      key: "project",
      header: "Project",
      width: "140px",
      render: (r: Report) => r.projectName,
    },
    {
      key: "scan",
      header: "Scan",
      width: "160px",
      render: (r: Report) => r.scanName,
    },
    {
      key: "format",
      header: "Format",
      width: "90px",
      render: (r: Report) => <Badge variant="info">{r.format.toUpperCase()}</Badge>,
    },
    {
      key: "findings",
      header: "Findings",
      width: "90px",
      render: (r: Report) => r.findingCount,
    },
    {
      key: "status",
      header: "Status",
      width: "110px",
      render: (r: Report) => <StatusBadge status={r.status} />,
    },
    {
      key: "created",
      header: "Generated",
      width: "170px",
      render: (r: Report) => new Date(r.createdAt).toLocaleString(),
    },
    {
      key: "actions",
      header: "",
      width: "120px",
      render: (r: Report) =>
        r.status === "completed" ? (
          <Button
            size="sm"
            variant="ghost"
            disabled={busyKey === r.id}
            onClick={() => handleStoredExport(r)}
          >
            {busyKey === r.id ? "…" : "Download"}
          </Button>
        ) : (
          <span className="text-muted">—</span>
        ),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Reports"
        description="Read-only report exports generated from SQLite findings via aisec-report"
        actions={
          <Button variant="secondary" onClick={() => void actions.refresh()} disabled={loading}>
            Refresh
          </Button>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <section className="reports-section">
        <div className="reports-section__header">
          <h2 className="reports-section__title">Export reports</h2>
          <span className="text-muted text-sm">
            Generate HTML, PDF, or SARIF from scan findings
          </span>
        </div>

        {exportRows.length === 0 && !loading ? (
          <Card>
            <EmptyState
              title="No scans with findings"
              description="Complete a scan with findings before exporting reports."
            />
          </Card>
        ) : (
          <Card padding="none">
            <DataTable
              columns={exportColumns}
              rows={exportRows}
              keyField="scanId"
              emptyMessage={loading ? "Loading scans…" : "No exportable scans"}
            />
          </Card>
        )}
      </section>

      <section className="reports-section">
        <div className="reports-section__header">
          <h2 className="reports-section__title">Stored reports</h2>
          <span className="text-muted text-sm">{sortedReports.length} in SQLite</span>
        </div>

        {sortedReports.length === 0 && !loading ? (
          <Card>
            <p className="text-muted">No reports stored yet. Export a format above to create one.</p>
          </Card>
        ) : (
          <Card padding="none">
            <DataTable
              columns={archiveColumns}
              rows={sortedReports}
              keyField="id"
              emptyMessage={loading ? "Loading reports…" : "No stored reports"}
            />
          </Card>
        )}
      </section>
    </div>
  );
}
