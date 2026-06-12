import { useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  SearchInput,
  SeverityBadge,
} from "@/shared/components";
import type { Finding, Severity } from "@/shared/types";

import { filterFindings } from "./findingsFilters";

const severities: Severity[] = ["critical", "high", "medium", "low", "info"];
const statuses: Finding["status"][] = ["open", "confirmed", "false_positive", "fixed"];

function truncate(text: string, max = 96): string {
  if (!text) return "—";
  if (text.length <= max) return text;
  return `${text.slice(0, max)}…`;
}

export function FindingsPage() {
  const {
    findings,
    projects,
    scans,
    loading,
    error,
    actions,
    dispatch,
    ui,
  } = useAppStore();
  const [scanFilter, setScanFilter] = useState<string>("");
  const [statusFilter, setStatusFilter] = useState<Finding["status"] | "">("");

  const filtered = useMemo(
    () =>
      filterFindings(
        findings,
        {
          searchQuery: ui.searchQuery,
          projectId: ui.selectedProjectId,
          scanId: scanFilter || null,
          severity: (ui.severityFilter as Severity | null) ?? null,
          status: statusFilter || null,
        },
        projects,
        scans,
      ),
    [
      findings,
      ui.searchQuery,
      ui.selectedProjectId,
      ui.severityFilter,
      scanFilter,
      statusFilter,
      projects,
      scans,
    ],
  );

  const projectName = (projectId: string) =>
    projects.find((p) => p.id === projectId)?.name ?? "—";

  const scanName = (scanId: string) =>
    scans.find((s) => s.id === scanId)?.name ?? scanId.slice(0, 8);

  const scanOptions = useMemo(() => {
    const ids = new Set(findings.map((f) => f.scanId));
    return scans.filter((scan) => ids.has(scan.id));
  }, [findings, scans]);

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
          <div className="text-muted text-sm">{truncate(f.description)}</div>
        </div>
      ),
    },
    {
      key: "project",
      header: "Project",
      width: "150px",
      render: (f: Finding) => projectName(f.projectId),
    },
    {
      key: "scan",
      header: "Scan",
      width: "160px",
      render: (f: Finding) => (
        <span className="text-sm" title={f.scanId}>
          {scanName(f.scanId)}
        </span>
      ),
    },
    {
      key: "target",
      header: "Target",
      width: "150px",
      render: (f: Finding) => f.targetName || "—",
    },
    {
      key: "category",
      header: "Category",
      width: "130px",
      render: (f: Finding) => (
        <Badge variant="muted">{f.category.replace(/_/g, " ")}</Badge>
      ),
    },
    {
      key: "verdict",
      header: "Verdict",
      width: "120px",
      render: (f: Finding) =>
        f.verdict === null ? (
          <span className="text-muted">—</span>
        ) : (
          <Badge variant={f.verdict === "vulnerable" ? "danger" : "muted"}>
            {f.verdict === "vulnerable" ? "Vulnerable" : "Not vulnerable"}
          </Badge>
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
      width: "120px",
      render: (f: Finding) => <Badge variant="muted">{f.status.replace(/_/g, " ")}</Badge>,
    },
  ];

  const hasActiveFilters =
    ui.searchQuery.trim() !== "" ||
    ui.selectedProjectId !== null ||
    ui.severityFilter !== null ||
    scanFilter !== "" ||
    statusFilter !== "";

  function clearFilters() {
    dispatch({ type: "SET_SEARCH", query: "" });
    dispatch({ type: "SET_SELECTED_PROJECT", projectId: null });
    dispatch({ type: "SET_SEVERITY_FILTER", severity: null });
    setScanFilter("");
    setStatusFilter("");
  }

  return (
    <div className="page">
      <PageHeader
        title="Findings"
        description="Read-only view of vulnerabilities from SQLite attack and judge results"
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

      <Card className="findings-toolbar">
        <div className="findings-toolbar__row">
          <SearchInput
            value={ui.searchQuery}
            onChange={(query) => dispatch({ type: "SET_SEARCH", query })}
            placeholder="Search title, target, category, project, scan…"
          />
          <label className="field findings-toolbar__field">
            <span className="field__label">Project</span>
            <select
              className="input"
              value={ui.selectedProjectId ?? ""}
              onChange={(e) =>
                dispatch({
                  type: "SET_SELECTED_PROJECT",
                  projectId: e.target.value || null,
                })
              }
            >
              <option value="">All projects</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>
          <label className="field findings-toolbar__field">
            <span className="field__label">Scan</span>
            <select
              className="input"
              value={scanFilter}
              onChange={(e) => setScanFilter(e.target.value)}
            >
              <option value="">All scans</option>
              {scanOptions.map((scan) => (
                <option key={scan.id} value={scan.id}>
                  {scan.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        <div className="findings-toolbar__filters">
          <span className="findings-toolbar__label">Severity</span>
          <div className="filter-bar findings-toolbar__chips">
            <button
              type="button"
              className={`filter-chip ${ui.severityFilter === null ? "filter-chip--active" : ""}`}
              onClick={() => dispatch({ type: "SET_SEVERITY_FILTER", severity: null })}
            >
              All
            </button>
            {severities.map((severity) => (
              <button
                key={severity}
                type="button"
                className={`filter-chip ${ui.severityFilter === severity ? "filter-chip--active" : ""}`}
                onClick={() => dispatch({ type: "SET_SEVERITY_FILTER", severity })}
              >
                {severity}
              </button>
            ))}
          </div>
        </div>

        <div className="findings-toolbar__filters">
          <span className="findings-toolbar__label">Status</span>
          <div className="filter-bar findings-toolbar__chips">
            <button
              type="button"
              className={`filter-chip ${statusFilter === "" ? "filter-chip--active" : ""}`}
              onClick={() => setStatusFilter("")}
            >
              All
            </button>
            {statuses.map((status) => (
              <button
                key={status}
                type="button"
                className={`filter-chip ${statusFilter === status ? "filter-chip--active" : ""}`}
                onClick={() => setStatusFilter(status)}
              >
                {status.replace(/_/g, " ")}
              </button>
            ))}
          </div>
        </div>

        <div className="findings-toolbar__meta">
          <span className="text-muted text-sm">
            {loading
              ? "Loading findings…"
              : `${filtered.length} of ${findings.length} finding${findings.length === 1 ? "" : "s"}`}
          </span>
          {hasActiveFilters && (
            <button type="button" className="findings-toolbar__clear" onClick={clearFilters}>
              Clear filters
            </button>
          )}
        </div>
      </Card>

      {findings.length === 0 && !loading ? (
        <EmptyState
          title="No findings yet"
          description="Run a scan from the wizard to populate findings in SQLite."
        />
      ) : (
        <Card padding="none">
          <DataTable
            columns={columns}
            rows={filtered}
            keyField="id"
            emptyMessage="No findings match your filters"
          />
        </Card>
      )}
    </div>
  );
}
