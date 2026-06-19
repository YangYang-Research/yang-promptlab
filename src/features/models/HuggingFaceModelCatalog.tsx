import { useState } from "react";

import { Button } from "@/shared/components";
import type { ModelCatalogEntryDto } from "@/shared/ipc/models";

import { huggingFaceModelIcon, huggingFaceRepoUrl } from "./huggingFace";

type HuggingFaceModelCatalogProps = {
  catalog: ModelCatalogEntryDto[];
  installedNames: Set<string>;
  downloadingId: string | null;
  installingId: string | null;
  backendConnected: boolean;
  onInstall: (entry: ModelCatalogEntryDto) => void;
};

export function HuggingFaceModelCatalog({
  catalog,
  installedNames,
  downloadingId,
  installingId,
  backendConnected,
  onInstall,
}: HuggingFaceModelCatalogProps) {
  const [selectedId, setSelectedId] = useState<string | null>(catalog[0]?.id ?? null);
  const selected = catalog.find((entry) => entry.id === selectedId) ?? null;
  const repoUrl = selected ? huggingFaceRepoUrl(selected.downloadUrl) : null;

  return (
    <div className="hf-catalog">
      <div className="hf-catalog__grid">
        {catalog.map((entry) => {
          const alreadyAdded = installedNames.has(entry.name);
          const isSelected = entry.id === selectedId;
          return (
            <button
              key={entry.id}
              type="button"
              className={`hf-catalog__card ${isSelected ? "hf-catalog__card--selected" : ""}`}
              onClick={() => setSelectedId(entry.id)}
            >
              <span className="hf-catalog__icon" aria-hidden="true">
                {huggingFaceModelIcon(entry.name)}
              </span>
              <span className="hf-catalog__name">{entry.name}</span>
              <span className="hf-catalog__desc">{entry.description}</span>
              <span
                className="hf-catalog__add"
                role="button"
                tabIndex={0}
                aria-label={`Add ${entry.name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  if (!alreadyAdded && backendConnected) onInstall(entry);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!alreadyAdded && backendConnected) onInstall(entry);
                  }
                }}
              >
                {alreadyAdded ? "✓" : "+"}
              </span>
            </button>
          );
        })}
      </div>

      {selected && (
        <div className="hf-catalog__detail">
          <h4 className="hf-catalog__detail-title">{selected.name}</h4>
          <p className="text-muted text-sm">{selected.description}</p>
          <dl className="hf-catalog__meta">
            <div>
              <dt>Provider</dt>
              <dd>{selected.provider}</dd>
            </div>
            <div>
              <dt>Purpose</dt>
              <dd>{selected.purpose}</dd>
            </div>
            <div>
              <dt>Engine</dt>
              <dd>
                {selected.engine} · {selected.format}
                {selected.quant ? ` · ${selected.quant}` : ""}
              </dd>
            </div>
            <div>
              <dt>Size</dt>
              <dd>
                {selected.sizeLabel ??
                  (selected.sizeGb != null ? `${selected.sizeGb.toFixed(1)} GB` : "—")}
              </dd>
            </div>
          </dl>
          {repoUrl && (
            <p className="text-sm">
              <a href={repoUrl} target="_blank" rel="noreferrer" className="link">
                View on Hugging Face →
              </a>
            </p>
          )}
          <div className="hf-catalog__detail-actions">
            <Button
              variant="secondary"
              disabled={
                !backendConnected ||
                installingId !== null ||
                downloadingId !== null ||
                installedNames.has(selected.name)
              }
              onClick={() => onInstall(selected)}
            >
              {installedNames.has(selected.name)
                ? "Added"
                : downloadingId === selected.id
                  ? "Downloading…"
                  : installingId === selected.id
                    ? "Starting…"
                    : "Add Model"}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
