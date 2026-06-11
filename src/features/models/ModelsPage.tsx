import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  PageHeader,
  ProgressBar,
  StatusBadge,
} from "@/shared/components";

export function ModelsPage() {
  const { models } = useAppStore();

  return (
    <div className="page">
      <PageHeader
        title="Models"
        description="Local GGUF models for offline judge and attacker roles"
        actions={
          <>
            <Button variant="ghost">Browse HuggingFace</Button>
            <Button variant="primary">Download Model</Button>
          </>
        }
      />

      <div className="models-grid">
        {models.map((model) => (
          <Card key={model.id} className="model-card">
            <div className="model-card__header">
              <div>
                <h3 className="model-card__name">{model.name}</h3>
                <p className="text-muted text-sm">
                  {model.provider} · {model.quant} · {model.sizeGb} GB
                </p>
              </div>
              <StatusBadge status={model.status} />
            </div>

            {model.status === "downloading" && (
              <ProgressBar value={model.downloadProgress} label="Downloading" />
            )}

            {model.path && (
              <p className="model-card__path mono text-sm">{model.path}</p>
            )}

            {model.sha256 && (
              <p className="text-muted text-sm">SHA256: {model.sha256}</p>
            )}

            <div className="model-card__actions">
              {model.status === "installed" && (
                <>
                  <Button size="sm" variant="ghost">Verify</Button>
                  <Button size="sm" variant="ghost">Remove</Button>
                </>
              )}
              {model.status === "available" && (
                <Button size="sm" variant="primary">Download</Button>
              )}
              {model.status === "downloading" && (
                <Button size="sm" variant="ghost">Cancel</Button>
              )}
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
