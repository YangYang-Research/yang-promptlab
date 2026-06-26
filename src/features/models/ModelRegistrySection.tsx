import {
  ActionsDropdown,
  Badge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  ListCard,
  Pagination,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import type { ModelEntryDto } from "@/shared/ipc/models";
import { formatBytes } from "@/shared/utils/format";

function isThirdPartyModel(model: ModelEntryDto): boolean {
  return model.format === "api" || model.id.startsWith("remote-");
}

function registryDisplayName(model: ModelEntryDto): string {
  if (isThirdPartyModel(model)) {
    return model.version || model.name;
  }
  return model.name;
}

function formatModelSize(model: ModelEntryDto): string {
  if (model.sizeBytes != null) {
    return formatBytes(model.sizeBytes);
  }
  if (model.sizeGb > 0) {
    return `${model.sizeGb.toFixed(2)} GB`;
  }
  return "—";
}

function formatModelCapabilities(model: ModelEntryDto): string {
  const caps = [
    model.capabilities.chat && "Chat",
    model.capabilities.completion && "Completion",
    model.capabilities.embeddings && "Embeddings",
  ].filter(Boolean);
  return caps.length > 0 ? caps.join(", ") : "—";
}

function ModelTypeBadge({ model }: { model: ModelEntryDto }) {
  if (isThirdPartyModel(model)) {
    return <Badge variant="info">Third-party</Badge>;
  }
  return <Badge variant="muted">Local</Badge>;
}

function ModelStatusBadge({ model }: { model: ModelEntryDto }) {
  if (isThirdPartyModel(model)) {
    return (
      <Badge variant={model.verified ? "success" : "warning"}>
        {model.verified ? "Verified" : "Not Verified"}
      </Badge>
    );
  }
  return (
    <Badge variant={model.verified ? "success" : "warning"}>
      {model.verified ? "Installed" : "Unverified"}
    </Badge>
  );
}

function ModelRegistryBadges({ model }: { model: ModelEntryDto }) {
  return (
    <div className="model-registry-badges">
      <ModelTypeBadge model={model} />
      <ModelStatusBadge model={model} />
    </div>
  );
}

type ModelRegistrySectionProps = {
  models: ModelEntryDto[];
  isModelBusy: (modelId: string) => boolean;
  runtimeModelLoading: boolean;
  onTest: (model: ModelEntryDto) => void;
  onEdit: (model: ModelEntryDto) => void;
  onRemove: (modelId: string) => void;
};

export function ModelRegistrySection({
  models,
  isModelBusy,
  runtimeModelLoading,
  onTest,
  onEdit,
  onRemove,
}: ModelRegistrySectionProps) {
  const [viewMode, setViewMode] = useViewPreference("models-registry");
  const [pageSize, setPageSize] = usePageSizePreference("models-registry");
  const { page, setPage, pagination } = usePaginatedList(models, pageSize);

  const actionsDisabled = (modelId: string) => isModelBusy(modelId);
  const localActionsDisabled = (modelId: string) =>
    isModelBusy(modelId) || runtimeModelLoading;

  function renderTableActions(model: ModelEntryDto) {
    const thirdParty = isThirdPartyModel(model);
    const testRemoveDisabled = thirdParty
      ? actionsDisabled(model.id)
      : localActionsDisabled(model.id);

    return (
      <ActionsDropdown
        items={[
          {
            id: "edit",
            label: "Edit",
            onClick: () => onEdit(model),
            disabled: !thirdParty || actionsDisabled(model.id),
          },
          {
            id: "verify",
            label: "Verify",
            onClick: () => onTest(model),
            disabled: testRemoveDisabled,
          },
          {
            id: "remove",
            label: "Remove",
            onClick: () => onRemove(model.id),
            tone: "danger",
            disabled: testRemoveDisabled,
          },
        ]}
      />
    );
  }

  function renderListActions(model: ModelEntryDto) {
    if (isThirdPartyModel(model)) {
      return (
        <>
          <Button
            variant="ghost"
            size="sm"
            disabled={actionsDisabled(model.id)}
            onClick={() => onEdit(model)}
          >
            Edit
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={actionsDisabled(model.id)}
            onClick={() => onTest(model)}
          >
            Test
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={actionsDisabled(model.id)}
            onClick={() => onRemove(model.id)}
          >
            Remove
          </Button>
        </>
      );
    }
    return (
      <>
        <Button
          variant="ghost"
          size="sm"
          disabled={localActionsDisabled(model.id)}
          onClick={() => onTest(model)}
        >
          Test
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={localActionsDisabled(model.id)}
          onClick={() => onRemove(model.id)}
        >
          Remove
        </Button>
      </>
    );
  }

  const columns = [
    {
      key: "name",
      header: "Model",
      render: (model: ModelEntryDto) => (
        <div>
          <strong>{registryDisplayName(model)}</strong>
          <div className="text-muted text-sm mono">{model.path}</div>
        </div>
      ),
    },
    {
      key: "type",
      header: "Type",
      width: "110px",
      render: (model: ModelEntryDto) => <ModelTypeBadge model={model} />,
    },
    {
      key: "status",
      header: "Status",
      width: "120px",
      render: (model: ModelEntryDto) => <ModelStatusBadge model={model} />,
    },
    {
      key: "provider",
      header: "Provider",
      width: "120px",
      render: (model: ModelEntryDto) => model.provider,
    },
    {
      key: "size",
      header: "Size",
      width: "100px",
      render: (model: ModelEntryDto) => formatModelSize(model),
    },
    {
      key: "capabilities",
      header: "Capabilities",
      width: "160px",
      render: (model: ModelEntryDto) => (
        <span className="text-sm">{formatModelCapabilities(model)}</span>
      ),
    },
    {
      key: "actions",
      header: "",
      width: "56px",
      render: (model: ModelEntryDto) => (
        <span className="table-actions" onClick={(event) => event.stopPropagation()}>
          {renderTableActions(model)}
        </span>
      ),
    },
  ];

  return (
    <section className="runtime-section">
      <h2 className="runtime-section__title">Model Registry</h2>

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
            emptyMessage="No models registered yet."
          />
        </Card>
      ) : (
        <div className="list-card-grid">
          {pagination.items.map((model) => (
            <ListCard
              key={model.id}
              title={registryDisplayName(model)}
              status={<ModelRegistryBadges model={model} />}
              metadata={[
                { label: "Provider", value: model.provider },
                { label: "Version", value: model.version || "—" },
                { label: "Size", value: formatModelSize(model) },
                { label: "Capabilities", value: formatModelCapabilities(model) },
              ]}
              footerMeta={<span className="mono text-sm">{model.path}</span>}
              actions={renderListActions(model)}
            />
          ))}
          {pagination.items.length === 0 && (
            <Card>
              <p className="text-muted">No models registered yet.</p>
            </Card>
          )}
        </div>
      )}

      {models.length > 0 && (
        <Pagination
          page={page}
          totalItems={pagination.totalItems}
          rangeStart={pagination.rangeStart}
          rangeEnd={pagination.rangeEnd}
          totalPages={pagination.totalPages}
          onPageChange={setPage}
        />
      )}
    </section>
  );
}
