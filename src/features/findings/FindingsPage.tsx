import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  AttackCategoryBadge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  FindingStatusBadge,
  ListCard,
  MultiSelect,
  PageHeader,
  Pagination,
  RefreshButton,
  SearchInput,
  SeverityBadge,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import type { Finding, Severity } from "@/shared/types";

import { filterFindings } from "./findingsFilters";
import { ImportSarifModal } from "./ImportSarifModal";

const severities: Severity[] = ["critical", "high", "medium", "low", "info"];
const statuses: Finding["status"][] = ["open", "confirmed", "false_positive", "fixed"];

function formatStatus(status: Finding["status"]): string {
  return status.replace(/_/g, " ");
}

export function FindingsPage() {
  const { findings, projects, scans, ui, dispatch, loading, error, actions } = useAppStore();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const searchQuery = ui.searchQuery;
  const [projectIds, setProjectIds] = useState<string[]>([]);
  const [scanIds, setScanIds] = useState<string[]>([]);
  const [severityFilter, setSeverityFilter] = useState<Severity[]>([]);
  const [statusFilter, setStatusFilter] = useState<Finding["status"][]>([]);
  const [viewMode, setViewMode] = useViewPreference("findings");
  const [pageSize, setPageSize] = usePageSizePreference("findings");
  const [importOpen, setImportOpen] = useState(false);

  useEffect(() => {
    const projectId = searchParams.get("projectId");
    if (projectId) {
      setProjectIds((current) => (current.includes(projectId) ? current : [projectId]));
    }
    const scanId = searchParams.get("scanId");
    if (scanId) {
      setScanIds((current) => (current.includes(scanId) ? current : [scanId]));
    }
  }, [searchParams]);

  const filtered = useMemo(
    () =>
      filterFindings(
        findings,
        {
          searchQuery,
          projectIds,
          scanIds,
          severities: severityFilter,
          statuses: statusFilter,
        },
        projects,
        scans,
      ),
    [findings, searchQuery, projectIds, scanIds, severityFilter, statusFilter, projects, scans],
  );

  const { page, setPage, pagination } = usePaginatedList(filtered, pageSize);

  const openFinding = (findingId: string) => {
    navigate(`/findings/${findingId}`);
  };

  const projectName = (projectId: string) =>
    projects.find((p) => p.id === projectId)?.name ?? "—";

  const scanOptions = useMemo(() => {
    const ids = new Set(findings.map((f) => f.scanId));
    return scans
      .filter((scan) => ids.has(scan.id))
      .map((scan) => ({ value: scan.id, label: scan.name }));
  }, [findings, scans]);

  const projectOptions = useMemo(
    () => projects.map((project) => ({ value: project.id, label: project.name })),
    [projects],
  );

  const severityOptions = useMemo(
    () =>
      severities.map((severity) => ({
        value: severity,
        label: severity.charAt(0).toUpperCase() + severity.slice(1),
      })),
    [],
  );

  const statusOptions = useMemo(
    () => statuses.map((status) => ({ value: status, label: formatStatus(status) })),
    [],
  );

  const columns = [
    {
      key: "project",
      header: "Project",
      width: "140px",
      render: (f: Finding) => projectName(f.projectId),
    },
    {
      key: "target",
      header: "Target",
      width: "240px",
      render: (f: Finding) => (
        <span className="mono text-sm" title={f.targetUrl || f.targetName || undefined}>
          {f.targetUrl || f.targetName || "—"}
        </span>
      ),
    },
    {
      key: "category",
      header: "Attack Category",
      width: "150px",
      render: (f: Finding) => <AttackCategoryBadge category={f.category} />,
    },
    {
      key: "title",
      header: "Finding",
      render: (f: Finding) => <strong>{f.title}</strong>,
    },
    {
      key: "severity",
      header: "Severity",
      width: "100px",
      render: (f: Finding) => <SeverityBadge severity={f.severity} />,
    },
    {
      key: "confidence",
      header: "Conf.",
      width: "72px",
      render: (f: Finding) => `${Math.round(f.confidence * 100)}%`,
    },
    {
      key: "status",
      header: "Status",
      width: "110px",
      render: (f: Finding) => <FindingStatusBadge status={f.status} />,
    },
  ];

  const hasActiveFilters =
    searchQuery.trim() !== "" ||
    projectIds.length > 0 ||
    scanIds.length > 0 ||
    severityFilter.length > 0 ||
    statusFilter.length > 0;

  function clearFilters() {
    dispatch({ type: "SET_SEARCH", query: "" });
    setProjectIds([]);
    setScanIds([]);
    setSeverityFilter([]);
    setStatusFilter([]);
  }

  return (
    <div className="page">
      <PageHeader
        title="Findings"
        description="Vulnerabilities from scans, judge results, and imported SARIF reports"
        actions={
          <div className="page-actions">
            <RefreshButton loading={loading} error={error} onClick={() => void actions.refresh()} />
            <Button variant="primary" onClick={() => setImportOpen(true)}>
              Import Finding
            </Button>
          </div>
        }
      />

      <ImportSarifModal open={importOpen} onClose={() => setImportOpen(false)} />
      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      {findings.length === 0 && !loading ? (
        <EmptyState
          title="No findings yet"
          description="Run a scan from the wizard, or import a SARIF report to get started."
        />
      ) : (
        <>
          <div className="findings-filters">
            <SearchInput
              value={searchQuery}
              onChange={(query) => dispatch({ type: "SET_SEARCH", query })}
              placeholder="Search title, target, category, project, scan…"
            />
            <MultiSelect
              label="Project"
              allLabel="All projects"
              options={projectOptions}
              values={projectIds}
              onChange={setProjectIds}
            />
            <MultiSelect
              label="Scan"
              allLabel="All scans"
              options={scanOptions}
              values={scanIds}
              onChange={setScanIds}
            />
            <MultiSelect
              label="Severity"
              allLabel="All severities"
              options={severityOptions}
              values={severityFilter}
              onChange={(values) => setSeverityFilter(values as Severity[])}
            />
            <MultiSelect
              label="Status"
              allLabel="All statuses"
              options={statusOptions}
              values={statusFilter}
              onChange={(values) => setStatusFilter(values as Finding["status"][])}
            />
            {hasActiveFilters && (
              <button type="button" className="findings-filters__clear" onClick={clearFilters}>
                Clear
              </button>
            )}
          </div>

          <ContentToolbar
            filters={
              <span className="text-muted text-sm">
                {loading
                  ? "Loading findings…"
                  : `${filtered.length} of ${findings.length} finding${findings.length === 1 ? "" : "s"}`}
              </span>
            }
            pageSize={pageSize}
            onPageSizeChange={setPageSize}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
          />

          {viewMode === "table" ? (
            <Card padding="none">
              <DataTable
                columns={columns}
                rows={pagination.items}
                keyField="id"
                emptyMessage={loading ? "Loading findings…" : "No findings match your filters"}
                loading={loading && pagination.items.length === 0}
                onRowClick={(finding) => openFinding(finding.id)}
              />
            </Card>
          ) : (
            <div className="list-card-grid">
              {pagination.items.length === 0 ? (
                <Card>
                  <EmptyState
                    title={loading ? "Loading findings…" : "No findings match your filters"}
                    description={
                      loading
                        ? "Fetching results from SQLite."
                        : "Try clearing filters or running another scan."
                    }
                  />
                </Card>
              ) : (
                pagination.items.map((finding) => (
                  <ListCard
                    key={finding.id}
                    title={finding.title}
                    status={<SeverityBadge severity={finding.severity} />}
                    metadata={[
                      { label: "Project", value: projectName(finding.projectId) },
                      {
                        label: "Target",
                        value: (
                          <span
                            className="mono text-sm"
                            title={finding.targetUrl || finding.targetName || undefined}
                          >
                            {finding.targetUrl || finding.targetName || "—"}
                          </span>
                        ),
                      },
                      {
                        label: "Attack Category",
                        value: <AttackCategoryBadge category={finding.category} />,
                      },
                      { label: "Conf.", value: `${Math.round(finding.confidence * 100)}%` },
                      {
                        label: "Status",
                        value: <FindingStatusBadge status={finding.status} />,
                      },
                    ]}
                    onClick={() => openFinding(finding.id)}
                  />
                ))
              )}
            </div>
          )}

          {filtered.length > 0 && (
            <Pagination
              page={page}
              totalItems={pagination.totalItems}
              rangeStart={pagination.rangeStart}
              rangeEnd={pagination.rangeEnd}
              totalPages={pagination.totalPages}
              onPageChange={setPage}
            />
          )}
        </>
      )}
    </div>
  );
}
