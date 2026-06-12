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
import type { Target } from "@/shared/types";

import { AddTargetModal } from "./AddTargetModal";

export function TargetsPage() {
  const { targets, projects, ui, loading, error, actions } = useAppStore();
  const [modalOpen, setModalOpen] = useState(false);

  const filtered = targets.filter((t) => {
    if (ui.selectedProjectId && t.projectId !== ui.selectedProjectId) return false;
    const q = ui.searchQuery.toLowerCase();
    if (!q) return true;
    return (
      t.name.toLowerCase().includes(q) ||
      t.url.toLowerCase().includes(q) ||
      t.tags.some((tag) => tag.includes(q))
    );
  });

  const projectName = (id: string) => projects.find((p) => p.id === id)?.name ?? "—";

  const columns = [
    {
      key: "name",
      header: "Target",
      render: (t: Target) => (
        <div>
          <strong>{t.name}</strong>
          <div className="text-muted text-sm mono">{t.url}</div>
        </div>
      ),
    },
    {
      key: "type",
      header: "Type",
      width: "80px",
      render: (t: Target) => <Badge variant="info">{t.type}</Badge>,
    },
    {
      key: "project",
      header: "Project",
      width: "180px",
      render: (t: Target) => projectName(t.projectId),
    },
    {
      key: "fingerprint",
      header: "Fingerprint",
      render: (t: Target) =>
        t.fingerprint ? (
          <span className="text-sm">{t.fingerprint}</span>
        ) : (
          <span className="text-muted">—</span>
        ),
    },
    {
      key: "status",
      header: "Status",
      width: "110px",
      render: (t: Target) => <StatusBadge status={t.status} />,
    },
    {
      key: "tags",
      header: "Tags",
      width: "140px",
      render: (t: Target) => (
        <div className="tag-list">
          {t.tags.map((tag) => (
            <Badge key={tag} variant="muted">{tag}</Badge>
          ))}
        </div>
      ),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Targets"
        description="Endpoints, applications, and models under test"
        actions={
          <>
            <Button variant="ghost" onClick={() => void actions.refresh()} disabled={loading}>
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
            <Button variant="primary" onClick={() => setModalOpen(true)}>
              Add Target
            </Button>
          </>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load targets: {error}</p>
        </Card>
      )}

      <Card padding="none">
        <DataTable
          columns={columns}
          rows={filtered}
          keyField="id"
          emptyMessage={loading ? "Loading targets…" : "No targets yet. Add your first target."}
        />
      </Card>

      <AddTargetModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </div>
  );
}
