import { useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  PageHeader,
} from "@/shared/components";
import type { Project } from "@/shared/types";

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function ProjectsPage() {
  const { projects, dispatch, ui, loading, error, actions } = useAppStore();
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const filtered = projects.filter((p) => {
    const q = ui.searchQuery.toLowerCase();
    if (!q) return true;
    return p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q);
  });

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed || submitting) return;
    setSubmitting(true);
    try {
      await actions.createProject(trimmed, description.trim() || null);
      setName("");
      setDescription("");
      setShowForm(false);
    } catch {
      // error surfaced via store.error
    } finally {
      setSubmitting(false);
    }
  }

  async function handleDelete(id: string) {
    try {
      await actions.deleteProject(id);
    } catch {
      // error surfaced via store.error
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
          <Button variant="danger" size="sm" onClick={() => handleDelete(p.id)}>
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
            <Button variant="primary" onClick={() => setShowForm((v) => !v)}>
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

      {showForm && (
        <Card>
          <form className="project-form" onSubmit={handleCreate}>
            <div className="project-form__row">
              <input
                className="input"
                placeholder="Project name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                autoFocus
              />
              <input
                className="input"
                placeholder="Description (optional)"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>
            <div className="project-form__actions">
              <Button variant="ghost" onClick={() => setShowForm(false)}>
                Cancel
              </Button>
              <Button variant="primary" type="submit" disabled={submitting || !name.trim()}>
                {submitting ? "Creating…" : "Create Project"}
              </Button>
            </div>
          </form>
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
    </div>
  );
}
