import { useEffect, useState } from "react";

import { Badge, Button } from "@/shared/components";
import type { ModelCatalogEntryDto } from "@/shared/ipc/models";
import { openExternalUrl } from "@/shared/utils/openExternalUrl";

import { huggingFaceModelIcon, huggingFaceRepoUrl } from "./huggingFace";

export const HF_CATALOG_IMPORT_ID = "__import__";

function formatCatalogCapabilities(entry: ModelCatalogEntryDto): string {
  const caps = [
    entry.capabilities.chat && "Chat",
    entry.capabilities.completion && "Completion",
    entry.capabilities.embeddings && "Embeddings",
  ].filter(Boolean);
  return caps.length > 0 ? caps.join(", ") : "—";
}

export type ImportModelFormProps = {
  backendConnected: boolean;
  importName: string;
  importPath: string;
  importBusy: "browse" | "import" | null;
  onImportNameChange: (value: string) => void;
  onImportPathChange: (value: string) => void;
  onBrowseImport: () => void;
  onImport: () => void;
};

type HuggingFaceModelCatalogProps = {
  catalog: ModelCatalogEntryDto[];
  installedNames: Set<string>;
  downloadingId: string | null;
  installingId: string | null;
  backendConnected: boolean;
  onInstall: (entry: ModelCatalogEntryDto) => void;
  importForm?: ImportModelFormProps;
  initialSelectImport?: boolean;
};

export function HuggingFaceModelCatalog({
  catalog,
  installedNames,
  downloadingId,
  installingId,
  backendConnected,
  onInstall,
  importForm,
  initialSelectImport = false,
}: HuggingFaceModelCatalogProps) {
  const [selectedId, setSelectedId] = useState<string | null>(
    initialSelectImport ? HF_CATALOG_IMPORT_ID : (catalog[0]?.id ?? null),
  );

  useEffect(() => {
    if (initialSelectImport) {
      setSelectedId(HF_CATALOG_IMPORT_ID);
      return;
    }
    setSelectedId((current) => {
      if (current === HF_CATALOG_IMPORT_ID) return current;
      if (current && catalog.some((entry) => entry.id === current)) return current;
      return catalog[0]?.id ?? null;
    });
  }, [catalog, initialSelectImport]);

  const selected = catalog.find((entry) => entry.id === selectedId) ?? null;
  const importSelected = selectedId === HF_CATALOG_IMPORT_ID;
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

        {importForm && (
          <button
            type="button"
            className={`hf-catalog__card hf-catalog__card--import ${importSelected ? "hf-catalog__card--selected" : ""}`}
            onClick={() => setSelectedId(HF_CATALOG_IMPORT_ID)}
          >
            <span className="hf-catalog__icon hf-catalog__icon--import" aria-hidden="true">
              ↑
            </span>
            <span className="hf-catalog__name">Import Model</span>
            <span className="hf-catalog__desc">Local .gguf or .zip package</span>
          </button>
        )}
      </div>

      {importSelected && importForm && (
        <ImportModelDetail {...importForm} />
      )}

      {selected && !importSelected && (
        <div className="hf-catalog__detail">
          <div className="hf-catalog__detail-header">
            <h4 className="hf-catalog__detail-title">{selected.name}</h4>
            {selected.recommended ? (
              <Badge variant="success">Recommended</Badge>
            ) : null}
          </div>
          <dl className="hf-catalog__meta">
            <div>
              <dt>Provider</dt>
              <dd>{selected.provider}</dd>
            </div>
            <div>
              <dt>Capabilities</dt>
              <dd>{formatCatalogCapabilities(selected)}</dd>
            </div>
            <div>
              <dt>Format</dt>
              <dd>{selected.format}</dd>
            </div>
            <div>
              <dt>Quantization</dt>
              <dd>{selected.quant ?? "—"}</dd>
            </div>
            <div>
              <dt>Engine</dt>
              <dd>{selected.engine}</dd>
            </div>
            <div>
              <dt>Size</dt>
              <dd>
                {selected.sizeLabel ??
                  (selected.sizeGb != null ? `${selected.sizeGb.toFixed(1)} GB` : "—")}
              </dd>
            </div>
          </dl>
          <div className="hf-catalog__detail-actions">
            {repoUrl && (
              <Button
                variant="secondary"
                onClick={() => void openExternalUrl(repoUrl)}
              >
                View on Hugging Face
              </Button>
            )}
            <Button
              variant="primary"
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

export function ImportModelDetail({
  backendConnected,
  importName,
  importPath,
  importBusy,
  onImportNameChange,
  onImportPathChange,
  onBrowseImport,
  onImport,
}: ImportModelFormProps) {
  return (
    <div className="hf-catalog__detail hf-catalog__detail--import">
      <h4 className="hf-catalog__detail-title">Import Model</h4>
      <p className="text-muted text-sm">
        Register a GGUF file or extract one from a ZIP package into the vault.
      </p>
      <div className="wizard-auth-fields">
        <div className="settings-field">
          <label htmlFor="addModelImportName">Display name</label>
          <input
            id="addModelImportName"
            className="input"
            value={importName}
            onChange={(e) => onImportNameChange(e.target.value)}
            disabled={!backendConnected || importBusy !== null}
          />
        </div>
        <div className="settings-field">
          <label htmlFor="addModelImportPath">Selected file</label>
          <div className="import-path-row">
            <input
              id="addModelImportPath"
              className="input mono"
              value={importPath}
              readOnly
              placeholder="Browse for a .gguf or .zip file"
              disabled={!backendConnected || importBusy !== null}
              onChange={(e) => onImportPathChange(e.target.value)}
            />
            <Button
              variant="secondary"
              disabled={!backendConnected || importBusy !== null}
              onClick={onBrowseImport}
            >
              {importBusy === "browse" ? "Opening…" : "Browse"}
            </Button>
          </div>
        </div>
      </div>
      <div className="hf-catalog__detail-actions">
        <Button
          variant="primary"
          disabled={!backendConnected || importBusy !== null}
          onClick={onImport}
        >
          {importBusy === "import" ? "Importing…" : "Import"}
        </Button>
      </div>
    </div>
  );
}
