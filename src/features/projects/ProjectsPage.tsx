import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  PageHeader,
} from "@/shared/components";
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
  const { projects, dispatch, ui, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const location = useLocation();
  const navigate = useNavigate();
  const [modalOpen, setModalOpen] = useState(false);

  useEffect(() => {
    const state = location.state as { openNewProject?: boolean } | null;
    if (state?.openNewProject) {
      setModalOpen(true);
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location, navigate]);

  const filtered = projects.filter((p) => {
    const q = ui.searchQuery.toLowerCase();
    if (!q) return true;
    return p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q);
  });

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
      render: (p: Project) => (
        <div>
          <strong>{p.name}</strong>
          <div className="text-muted text-sm">{p.description}</div>
        </div>
      ),
    },
    {
      key: "status",
      header: "Status",
      width: "100px",
      render: (p: Project) => (
        <Badge variant={p.status === "active" ? "success" : p.status === "draft" ? "muted" : "default"}>
          {p.status}
        </Badge>
      ),
    },
    {
      key: "targets",
      header: "Targets",
      width: "80px",
      render: (p: Project) => p.targetCount,
    },
    {
      key: "findings",
      header: "Findings",
      width: "90px",
      render: (p: Project) => p.findingCount,
    },
    {
      key: "updated",
      header: "Updated",
      width: "120px",
      render: (p: Project) => formatDate(p.updatedAt),
    },
    {
      key: "actions",
      header: "",
      width: "90px",
      render: (p: Project) => (
        <span onClick={(e) => e.stopPropagation()}>
          <Button variant="danger" size="sm" onClick={() => handleDelete(p)}>
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
            <Button variant="ghost" onClick={() => void actions.refresh()} disabled={loading}>
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
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

      <Card padding="none">
        <DataTable
          columns={columns}
          rows={filtered}
          keyField="id"
          onRowClick={(p) => dispatch({ type: "SET_SELECTED_PROJECT", projectId: p.id })}
          emptyMessage={loading ? "Loading projects…" : "No projects yet. Create your first project."}
        />
      </Card>

      <NewProjectModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </div>
  );
}
