import { useAppStore, useFilteredFindings } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  PageHeader,
  SeverityBadge,
} from "@/shared/components";
import type { Finding } from "@/shared/types";

const severities = ["critical", "high", "medium", "low", "info"] as const;

export function FindingsPage() {
  const { dispatch, ui } = useAppStore();
  const findings = useFilteredFindings();

  const columns = [
    {
      key: "severity",
      header: "Severity",
      width: "100px",
      render: (f: Finding) => <SeverityBadge severity={f.severity} />,
    },
    {
      key: "title",
      header: "Finding",
      render: (f: Finding) => (
        <div>
          <strong>{f.title}</strong>
          <div className="text-muted text-sm">{f.description.slice(0, 80)}…</div>
        </div>
      ),
    },
    {
      key: "target",
      header: "Target",
      width: "180px",
      render: (f: Finding) => f.targetName,
    },
    {
      key: "category",
      header: "Category",
      width: "140px",
      render: (f: Finding) => (
        <Badge variant="muted">{f.category.replace(/_/g, " ")}</Badge>
      ),
    },
    {
      key: "confidence",
      header: "Confidence",
      width: "100px",
      render: (f: Finding) => `${Math.round(f.confidence * 100)}%`,
    },
    {
      key: "status",
      header: "Status",
      width: "130px",
      render: (f: Finding) => (
        <select
          className="select-inline"
          value={f.status}
          onChange={(e) =>
            dispatch({
              type: "UPDATE_FINDING_STATUS",
              findingId: f.id,
              status: e.target.value as Finding["status"],
            })
          }
          onClick={(e) => e.stopPropagation()}
        >
          <option value="open">open</option>
          <option value="confirmed">confirmed</option>
          <option value="false_positive">false positive</option>
          <option value="fixed">fixed</option>
        </select>
      ),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Findings"
        description="Vulnerabilities discovered during attack and judge evaluation"
        actions={
          <>
            <Button variant="ghost">Export SARIF</Button>
            <Button variant="primary">Triage</Button>
          </>
        }
      />

      <div className="filter-bar">
        <button
          type="button"
          className={`filter-chip ${ui.severityFilter === null ? "filter-chip--active" : ""}`}
          onClick={() => dispatch({ type: "SET_SEVERITY_FILTER", severity: null })}
        >
          All
        </button>
        {severities.map((sev) => (
          <button
            key={sev}
            type="button"
            className={`filter-chip ${ui.severityFilter === sev ? "filter-chip--active" : ""}`}
            onClick={() => dispatch({ type: "SET_SEVERITY_FILTER", severity: sev })}
          >
            {sev}
          </button>
        ))}
      </div>

      <Card padding="none">
        <DataTable
          columns={columns}
          rows={findings}
          keyField="id"
          emptyMessage="No findings match your filters"
        />
      </Card>
    </div>
  );
}
