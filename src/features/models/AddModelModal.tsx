import { useEffect, useState, type ComponentType } from "react";

import { IconCloud, IconImport, IconOnDevice, Modal } from "@/shared/components";
import type { ModelCatalogEntryDto } from "@/shared/ipc/models";

import { HuggingFaceModelCatalog, ImportModelDetail } from "./HuggingFaceModelCatalog";
import { ThirdPartyModelsPanel } from "./ThirdPartyModelsPanel";
import type { ThirdPartyModelForm } from "@/shared/ipc/thirdPartyModels";

export type AddModelTab = "public" | "third-party" | "import";

type AddModelModalProps = {
  open: boolean;
  onClose: () => void;
  initialTab?: AddModelTab;
  initialThirdPartyForm?: ThirdPartyModelForm | null;
  editingModelId?: string | null;
  modalTitle?: string;
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

const TABS: Array<{
  id: AddModelTab;
  navLabel: string;
  panelTitle: string;
  hint: string;
  icon: ComponentType<{ className?: string }>;
}> = [
  {
    id: "public",
    navLabel: "Catalog",
    panelTitle: "Public models",
    hint: "Download curated GGUF models from Hugging Face for on-device inference.",
    icon: IconOnDevice,
  },
  {
    id: "import",
    navLabel: "Import",
    panelTitle: "Import model",
    hint: "Add a local .gguf (or .zip) file already on this machine into the vault.",
    icon: IconImport,
  },
  {
    id: "third-party",
    navLabel: "Remote",
    panelTitle: "Third-party providers",
    hint: "Connect cloud LLM APIs for use with AI Runtime.",
    icon: IconCloud,
  },
];

function resolveTab(initialTab?: AddModelTab): AddModelTab {
  if (initialTab === "third-party" || initialTab === "import") return initialTab;
  return "public";
}

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
  initialTab,
  initialThirdPartyForm = null,
  editingModelId = null,
  modalTitle = "Add Model",
}: AddModelModalProps) {
  const [tab, setTab] = useState<AddModelTab>(resolveTab(initialTab));

  useEffect(() => {
    if (!open) return;
    setTab(resolveTab(initialTab));
  }, [open, initialTab]);

  const isEditing = Boolean(editingModelId);
  const selectedTab = TABS.find((entry) => entry.id === tab) ?? TABS[0];

  const importFormProps = {
    backendConnected,
    importName,
    importPath,
    importBusy,
    onImportNameChange,
    onImportPathChange,
    onBrowseImport,
    onImport,
  };

  if (isEditing) {
    return (
      <Modal open={open} title={modalTitle} onClose={onClose} size="medium">
        <ThirdPartyModelsPanel
          key={open ? `edit-${editingModelId}` : "edit-closed"}
          backendConnected={backendConnected}
          initialForm={initialThirdPartyForm}
          editingModelId={editingModelId}
          onSaved={() => {
            onThirdPartySaved();
            onClose();
          }}
        />
      </Modal>
    );
  }

  return (
    <Modal open={open} title={modalTitle} onClose={onClose} size="wide">
      <div className="add-model-modal">
        <div className="add-model-modal__layout">
          <nav className="add-model-modal__nav" aria-label="Add model sources">
            {TABS.map((entry) => {
              const NavIcon = entry.icon;
              return (
                <button
                  key={entry.id}
                  type="button"
                  className={`add-model-modal__nav-item ${tab === entry.id ? "add-model-modal__nav-item--active" : ""}`}
                  onClick={() => setTab(entry.id)}
                >
                  <NavIcon className="add-model-modal__nav-icon" />
                  <span>{entry.navLabel}</span>
                </button>
              );
            })}
          </nav>

          <div className="add-model-modal__panel">
            <div className="detail-section__header add-model-modal__panel-intro">
              <div>
                <h3 className="add-model-modal__panel-title">{selectedTab.panelTitle}</h3>
                <p className="detail-section__hint">{selectedTab.hint}</p>
              </div>
            </div>

            {tab === "public" && (
              <HuggingFaceModelCatalog
                catalog={catalog}
                installedNames={installedNames}
                downloadingId={downloadingId}
                installingId={installingId}
                backendConnected={backendConnected}
                onInstall={onInstall}
              />
            )}

            {tab === "import" && <ImportModelDetail {...importFormProps} />}

            {tab === "third-party" && (
              <ThirdPartyModelsPanel
                key={
                  open
                    ? `third-party-panel-${initialThirdPartyForm?.provider ?? "new"}-${initialThirdPartyForm?.model ?? ""}`
                    : "third-party-panel-closed"
                }
                backendConnected={backendConnected}
                initialForm={initialThirdPartyForm}
                editingModelId={editingModelId}
                onSaved={() => {
                  onThirdPartySaved();
                  onClose();
                }}
              />
            )}
          </div>
        </div>
      </div>
    </Modal>
  );
}
