import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  PageHeader,
  StatusBadge,
} from "@/shared/components";
import type { Report } from "@/shared/types";

function formatSize(bytes: number) {
  if (bytes === 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function ReportsPage() {
  const { reports } = useAppStore();

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
      key: "size",
      header: "Size",
      width: "90px",
      render: (r: Report) => formatSize(r.sizeBytes),
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
      width: "100px",
      render: (r: Report) =>
        r.status === "completed" ? (
          <Button size="sm" variant="ghost">Download</Button>
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
            <Button variant="ghost">Templates</Button>
            <Button variant="primary">Generate Report</Button>
          </>
        }
      />

      <div className="report-format-grid">
        {(["pdf", "html", "json", "sarif", "markdown"] as const).map((fmt) => (
          <Card key={fmt} className="report-format-card" padding="sm">
            <Badge variant="info">{fmt.toUpperCase()}</Badge>
            <p className="text-sm text-muted">
              {fmt === "pdf" && "Executive summary with charts"}
              {fmt === "html" && "Interactive technical report"}
              {fmt === "json" && "Machine-readable findings export"}
              {fmt === "sarif" && "CI/CD integration format"}
              {fmt === "markdown" && "Lightweight summary for wikis"}
            </p>
          </Card>
        ))}
      </div>

      <Card padding="none">
        <DataTable
          columns={columns}
          rows={reports}
          keyField="id"
          emptyMessage="No reports generated yet"
        />
      </Card>
    </div>
  );
}
