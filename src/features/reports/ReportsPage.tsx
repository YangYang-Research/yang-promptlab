import { useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  PageHeader,
  StatusBadge,
} from "@/shared/components";
import { readReport } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { Report } from "@/shared/types";

import { GenerateReportModal } from "./GenerateReportModal";

export function ReportsPage() {
  const { reports, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [modalOpen, setModalOpen] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  async function downloadReport(report: Report) {
    setBusyId(report.id);
    try {
      const file = await readReport(report.id);
      const ext = file.format || "html";
      const mime = ext === "html" ? "text/html" : "text/plain";
      const blob = new Blob([file.content], { type: mime });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${file.name.replace(/[^a-z0-9-_]+/gi, "_")}.${ext}`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      notify("Report downloaded", "success");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to read report";
      notify(message, "error");
    } finally {
      setBusyId(null);
    }
  }

  async function viewReport(report: Report) {
    setBusyId(report.id);
    try {
      const file = await readReport(report.id);
      const blob = new Blob([file.content], { type: "text/html" });
      const url = URL.createObjectURL(blob);
      window.open(url, "_blank");
      // URL is intentionally not revoked immediately so the new tab can load it.
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to open report";
      notify(message, "error");
    } finally {
      setBusyId(null);
    }
  }

  const columns = [
    {
      key: "title",
      header: "Report",
      render: (r: Report) => (
        <div>
          <strong>{r.title}</strong>
          <div className="text-muted text-sm">{r.projectName}</div>
        </div>
      ),
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
      header: "Created",
      width: "160px",
      render: (r: Report) => new Date(r.createdAt).toLocaleString(),
    },
    {
      key: "actions",
      header: "",
      width: "170px",
      render: (r: Report) =>
        r.status === "completed" ? (
          <span className="row-actions" onClick={(e) => e.stopPropagation()}>
            <Button size="sm" variant="ghost" onClick={() => viewReport(r)} disabled={busyId === r.id}>
              View
            </Button>
            <Button
              size="sm"
              variant="primary"
              onClick={() => downloadReport(r)}
              disabled={busyId === r.id}
            >
              {busyId === r.id ? "…" : "Download"}
            </Button>
          </span>
        ) : (
          <span className="text-muted">—</span>
        ),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Reports"
        description="Executive, technical, and compliance report generation"
        actions={
          <>
            <Button variant="ghost" onClick={() => void actions.refresh()} disabled={loading}>
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
            <Button variant="primary" onClick={() => setModalOpen(true)}>
              Generate Report
            </Button>
          </>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load reports: {error}</p>
        </Card>
      )}

      <Card padding="none">
        <DataTable
          columns={columns}
          rows={reports}
          keyField="id"
          emptyMessage={loading ? "Loading reports…" : "No reports generated yet. Generate your first report."}
        />
      </Card>

      <GenerateReportModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </div>
  );
}
