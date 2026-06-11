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
  const { projects, dispatch, ui } = useAppStore();

  const filtered = projects.filter((p) => {
    const q = ui.searchQuery.toLowerCase();
    if (!q) return true;
    return p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q);
  });

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
      key: "owner",
      header: "Owner",
      width: "160px",
      render: (p: Project) => <span className="text-muted">{p.owner}</span>,
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Projects"
        description="Organize security assessments by engagement or product"
        actions={
          <>
            <Button variant="ghost">Import</Button>
            <Button variant="primary">New Project</Button>
          </>
        }
      />

      <Card padding="none">
        <DataTable
          columns={columns}
          rows={filtered}
          keyField="id"
          onRowClick={(p) => dispatch({ type: "SET_SELECTED_PROJECT", projectId: p.id })}
          emptyMessage="No projects match your search"
        />
      </Card>
    </div>
  );
}
