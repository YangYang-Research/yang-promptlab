import { useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  PageHeader,
  Pagination,
  RefreshButton,
  StatusBadge,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
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

const EXPORT_FORMATS: ReportExportFormat[] = ["html", "pdf", "sarif", "csv"];

export function ReportsPage() {
  const { reports, scans, findings, projects, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [exportPageSize, setExportPageSize] = usePageSizePreference("reports-export");
  const [archivePageSize, setArchivePageSize] = usePageSizePreference("reports-archive");

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

  const {
    page: exportPage,
    setPage: setExportPage,
    pagination: exportPagination,
  } = usePaginatedList(exportRows, exportPageSize);

  const {
    page: archivePage,
    setPage: setArchivePage,
    pagination: archivePagination,
  } = usePaginatedList(sortedReports, archivePageSize);

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
        description="Read-only report exports generated from SQLite findings via promptlab-report"
        actions={
          <RefreshButton loading={loading} onClick={() => void actions.refresh()} />
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <section className="reports-section">
        <div className="reports-section__header">
          <div>
            <h2 className="reports-section__title">Export reports</h2>
            <span className="text-muted text-sm">
              Generate HTML, PDF, or SARIF from scan findings
            </span>
          </div>
          {exportRows.length > 0 && (
            <ContentToolbar
              pageSize={exportPageSize}
              onPageSizeChange={setExportPageSize}
              showViewMode={false}
            />
          )}
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
              rows={exportPagination.items}
              keyField="scanId"
              emptyMessage={loading ? "Loading scans…" : "No exportable scans"}
              loading={loading && exportPagination.items.length === 0}
            />
          </Card>
        )}

        {exportRows.length > 0 && (
          <Pagination
            page={exportPage}
            totalItems={exportPagination.totalItems}
            rangeStart={exportPagination.rangeStart}
            rangeEnd={exportPagination.rangeEnd}
            totalPages={exportPagination.totalPages}
            onPageChange={setExportPage}
          />
        )}
      </section>

      <section className="reports-section">
        <div className="reports-section__header">
          <div>
            <h2 className="reports-section__title">Stored reports</h2>
            <span className="text-muted text-sm">{sortedReports.length} in SQLite</span>
          </div>
          {sortedReports.length > 0 && (
            <ContentToolbar
              pageSize={archivePageSize}
              onPageSizeChange={setArchivePageSize}
              showViewMode={false}
            />
          )}
        </div>

        {sortedReports.length === 0 && !loading ? (
          <Card>
            <p className="text-muted">No reports stored yet. Export a format above to create one.</p>
          </Card>
        ) : (
          <Card padding="none">
            <DataTable
              columns={archiveColumns}
              rows={archivePagination.items}
              keyField="id"
              emptyMessage={loading ? "Loading reports…" : "No stored reports"}
              loading={loading && archivePagination.items.length === 0}
            />
          </Card>
        )}

        {sortedReports.length > 0 && (
          <Pagination
            page={archivePage}
            totalItems={archivePagination.totalItems}
            rangeStart={archivePagination.rangeStart}
            rangeEnd={archivePagination.rangeEnd}
            totalPages={archivePagination.totalPages}
            onPageChange={setArchivePage}
          />
        )}
      </section>
    </div>
  );
}
