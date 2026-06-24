import { useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  ListCard,
  PageHeader,
  Pagination,
  RefreshButton,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { useToast } from "@/shared/notifications";
import type { Project } from "@/shared/types";

import { NewProjectModal } from "./NewProjectModal";

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function ProjectsPage() {
  const { projects, ui, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const location = useLocation();
  const navigate = useNavigate();
  const [modalOpen, setModalOpen] = useState(false);
  const [viewMode, setViewMode] = useViewPreference("projects");
  const [pageSize, setPageSize] = usePageSizePreference("projects");

  useEffect(() => {
    const state = location.state as { openNewProject?: boolean } | null;
    if (state?.openNewProject) {
      setModalOpen(true);
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location, navigate]);

  const filtered = useMemo(
    () =>
      projects.filter((project) => {
        const q = ui.searchQuery.toLowerCase();
        if (!q) return true;
        return (
          project.name.toLowerCase().includes(q) ||
          project.description.toLowerCase().includes(q)
        );
      }),
    [projects, ui.searchQuery],
  );

  const { page, setPage, pagination } = usePaginatedList(filtered, pageSize);

  async function handleDelete(project: Project) {
    try {
      await actions.deleteProject(project.id);
      notify(`Project "${project.name}" deleted`, "success");
    } catch {
      notify("Failed to delete project", "error");
    }
  }

  const columns = [
    {
      key: "name",
      header: "Project",
      render: (project: Project) => (
        <div>
          <strong>{project.name}</strong>
          <div className="text-muted text-sm">{project.description}</div>
        </div>
      ),
    },
    {
      key: "status",
      header: "Status",
      width: "100px",
      render: (project: Project) => (
        <Badge
          variant={
            project.status === "active"
              ? "success"
              : project.status === "draft"
                ? "muted"
                : "default"
          }
        >
          {project.status}
        </Badge>
      ),
    },
    {
      key: "targets",
      header: "Targets",
      width: "80px",
      render: (project: Project) => project.targetCount,
    },
    {
      key: "findings",
      header: "Findings",
      width: "90px",
      render: (project: Project) => project.findingCount,
    },
    {
      key: "updated",
      header: "Updated",
      width: "120px",
      render: (project: Project) => formatDate(project.updatedAt),
    },
    {
      key: "actions",
      header: "",
      width: "80px",
      render: (project: Project) => (
        <span className="table-actions" onClick={(event) => event.stopPropagation()}>
          <Button variant="danger" size="sm" onClick={() => void handleDelete(project)}>
            Delete
          </Button>
        </span>
      ),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Projects"
        description="Organize security assessments by engagement or product"
        actions={
          <>
            <RefreshButton loading={loading} error={error} onClick={() => void actions.refresh()} />
            <Button variant="primary" onClick={() => setModalOpen(true)}>
              New Project
            </Button>
          </>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load projects: {error}</p>
        </Card>
      )}

      <ContentToolbar
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
            onRowClick={(project) => navigate(`/projects/${project.id}`)}
            emptyMessage={loading ? "Loading projects…" : "No projects yet. Create your first project."}
          />
        </Card>
      ) : (
        <div className="list-card-grid">
          {pagination.items.map((project) => (
            <ListCard
              key={project.id}
              title={project.name}
              status={
                <Badge
                  variant={
                    project.status === "active"
                      ? "success"
                      : project.status === "draft"
                        ? "muted"
                        : "default"
                  }
                >
                  {project.status}
                </Badge>
              }
              metadata={[
                { label: "Targets", value: project.targetCount },
                { label: "Findings", value: project.findingCount },
                { label: "Description", value: project.description || "—" },
              ]}
              footerMeta={`Updated: ${formatDate(project.updatedAt)}`}
              actions={
                <Button variant="danger" size="sm" onClick={() => void handleDelete(project)}>
                  Delete
                </Button>
              }
              onClick={() => navigate(`/projects/${project.id}`)}
            />
          ))}
          {pagination.items.length === 0 && (
            <Card>
              <p className="text-muted">
                {loading ? "Loading projects…" : "No projects yet. Create your first project."}
              </p>
            </Card>
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

      <NewProjectModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </div>
  );
}
