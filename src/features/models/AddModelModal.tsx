import { useState } from "react";

import { Button, Modal } from "@/shared/components";
import type { ModelCatalogEntryDto } from "@/shared/ipc/models";

import { HuggingFaceModelCatalog } from "./HuggingFaceModelCatalog";
import { ThirdPartyModelsPanel } from "./ThirdPartyModelsPanel";

export type AddModelTab = "public" | "third-party" | "import";

type AddModelModalProps = {
  open: boolean;
  onClose: () => void;
  backendConnected: boolean;
  catalog: ModelCatalogEntryDto[];
  installedNames: Set<string>;
  downloadingId: string | null;
  installingId: string | null;
  importName: string;
  importPath: string;
  importBusy: "browse" | "import" | null;
  onImportNameChange: (value: string) => void;
  onImportPathChange: (value: string) => void;
  onInstall: (entry: ModelCatalogEntryDto) => void;
  onBrowseImport: () => void;
  onImport: () => void;
  onThirdPartySaved: () => void;
};

const TABS: Array<{ id: AddModelTab; label: string; hint: string }> = [
  { id: "public", label: "Public Models", hint: "Built-in Hugging Face GGUF catalog" },
  { id: "third-party", label: "Third-party Models", hint: "Cloud LLM providers (Bedrock, OpenAI, …)" },
  { id: "import", label: "Import Model", hint: "Local .gguf or .zip package" },
];

export function AddModelModal({
  open,
  onClose,
  backendConnected,
  catalog,
  installedNames,
  downloadingId,
  installingId,
  importName,
  importPath,
  importBusy,
  onImportNameChange,
  onImportPathChange,
  onInstall,
  onBrowseImport,
  onImport,
  onThirdPartySaved,
}: AddModelModalProps) {
  const [tab, setTab] = useState<AddModelTab>("public");
  const selectedTab = TABS.find((entry) => entry.id === tab) ?? TABS[0];

  return (
    <Modal open={open} title="Add Model" onClose={onClose} size="wide">
      <div className="add-model-modal">
        <div className="add-model-modal__layout">
          <nav className="add-model-modal__nav" aria-label="Add model sources">
            {TABS.map((entry) => (
              <button
                key={entry.id}
                type="button"
                className={`add-model-modal__nav-item ${tab === entry.id ? "add-model-modal__nav-item--active" : ""}`}
                onClick={() => setTab(entry.id)}
              >
                {entry.label}
              </button>
            ))}
          </nav>

          <div className="add-model-modal__panel">
            <div className="add-model-modal__panel-header">
              <h3 className="add-model-modal__panel-title">{selectedTab.label}</h3>
              <p className="text-muted text-sm">{selectedTab.hint}</p>
            </div>

            {tab === "public" && (
              <>
                <p className="text-muted text-sm">
                  Built-in GGUF catalog from <code>resources/models.json</code>. Click a card for
                  details; use <strong>+</strong> or <strong>Add Model</strong> to download.
                </p>
                <HuggingFaceModelCatalog
                  catalog={catalog}
                  installedNames={installedNames}
                  downloadingId={downloadingId}
                  installingId={installingId}
                  backendConnected={backendConnected}
                  onInstall={onInstall}
                />
              </>
            )}

            {tab === "third-party" && (
              <ThirdPartyModelsPanel
                key={open ? "third-party-panel" : "third-party-panel-closed"}
                backendConnected={backendConnected}
                onSaved={() => {
                  onThirdPartySaved();
                  onClose();
                }}
              />
            )}

            {tab === "import" && (
              <>
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
                <div className="model-card__actions">
                  <Button
                    variant="primary"
                    disabled={!backendConnected || importBusy !== null}
                    onClick={onImport}
                  >
                    {importBusy === "import" ? "Importing…" : "Import"}
                  </Button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </Modal>
  );
}
