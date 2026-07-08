import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
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
import { assertAiRuntimeReady } from "@/shared/runtime/aiRuntimeReadiness";
import type { Project } from "@/shared/types";

import { NewProjectModal } from "./NewProjectModal";
import { EditProjectModal } from "./EditProjectModal";

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function ProjectsPage() {
  const { projects, ui, loading, error, actions, backendConnected } = useAppStore();
  const { notify } = useToast();
  const location = useLocation();
  const navigate = useNavigate();
  const [modalOpen, setModalOpen] = useState(false);
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [deletingProjectId, setDeletingProjectId] = useState<string | null>(null);
  const [openingProject, setOpeningProject] = useState(false);
  const [viewMode, setViewMode] = useViewPreference("projects");
  const [pageSize, setPageSize] = usePageSizePreference("projects");

  useEffect(() => {
    const state = location.state as { openNewProject?: boolean } | null;
    if (state?.openNewProject) {
      navigate(location.pathname, { replace: true, state: null });
      void openNewProjectModal();
    }
  }, [location, navigate]);

  async function openNewProjectModal() {
    if (openingProject) return;
    setOpeningProject(true);
    try {
      const readiness = await assertAiRuntimeReady(backendConnected);
      if (!readiness.ready) {
        notify(readiness.message, "error");
        navigate("/runtime");
        return;
      }
      setModalOpen(true);
    } finally {
      setOpeningProject(false);
    }
  }

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

  const handleDelete = useCallback(
    async (project: Project) => {
      const confirmed = window.confirm(
        `Delete project "${project.name}"? This cannot be undone.`,
      );
      if (!confirmed) return;

      setDeletingProjectId(project.id);
      try {
        await actions.deleteProject(project.id);
        notify(`Project "${project.name}" deleted`, "success");
      } catch {
        notify("Failed to delete project", "error");
      } finally {
        setDeletingProjectId(null);
      }
    },
    [actions, notify],
  );

  const buildProjectActionItems = useCallback(
    (project: Project): ActionsDropdownItem[] => [
      {
        id: "edit",
        label: "Edit Project",
        onClick: () => setEditingProject(project),
      },
      {
        id: "delete",
        label: "Delete Project",
        tone: "danger",
        disabled: deletingProjectId === project.id,
        onClick: () => void handleDelete(project),
      },
    ],
    [deletingProjectId, handleDelete],
  );

  const columns = useMemo(
    () => [
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
      width: "56px",
      render: (project: Project) => (
        <span onClick={(event) => event.stopPropagation()}>
          <ActionsDropdown
            label="Project actions"
            disabled={deletingProjectId === project.id}
            items={buildProjectActionItems(project)}
          />
        </span>
      ),
    },
  ],
    [buildProjectActionItems, deletingProjectId],
  );

  return (
    <div className="page">
      <PageHeader
        title="Projects"
        description="Organize security assessments by engagement or product"
        actions={
          <>
            <RefreshButton loading={loading} error={error} onClick={() => void actions.refresh()} />
            <Button
              variant="primary"
              disabled={openingProject}
              onClick={() => void openNewProjectModal()}
            >
              {openingProject ? "Checking AI Runtime…" : "New Project"}
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
            loading={loading && pagination.items.length === 0}
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
                <ActionsDropdown
                  label="Project actions"
                  disabled={deletingProjectId === project.id}
                  items={buildProjectActionItems(project)}
                />
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
      <EditProjectModal
        open={editingProject !== null}
        project={editingProject}
        onClose={() => setEditingProject(null)}
      />
    </div>
  );
}
