import { useEffect, useState, type ComponentType } from "react";

import { IconCloud, IconOnDevice, Modal } from "@/shared/components";
import type { ModelCatalogEntryDto } from "@/shared/ipc/models";

import { HuggingFaceModelCatalog } from "./HuggingFaceModelCatalog";
import { ThirdPartyModelsPanel } from "./ThirdPartyModelsPanel";
import type { ThirdPartyModelForm } from "@/shared/ipc/thirdPartyModels";

export type AddModelTab = "public" | "third-party" | "import";

type NavTab = Exclude<AddModelTab, "import">;

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
  id: NavTab;
  navLabel: string;
  panelTitle: string;
  hint: string;
  icon: ComponentType<{ className?: string }>;
}> = [
  {
    id: "public",
    navLabel: "On-device",
    panelTitle: "Public Models",
    hint: "Built-in Hugging Face GGUF catalog and local import",
    icon: IconOnDevice,
  },
  {
    id: "third-party",
    navLabel: "Remote",
    panelTitle: "Third-party Providers",
    hint: "Register cloud LLM providers for remote inference. Credentials are stored securely in the OS keychain when available.",
    icon: IconCloud,
  },
];

function resolveNavTab(initialTab?: AddModelTab): NavTab {
  if (initialTab === "third-party") return "third-party";
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
  const [tab, setTab] = useState<NavTab>(resolveNavTab(initialTab));
  const [selectImport, setSelectImport] = useState(initialTab === "import");

  useEffect(() => {
    if (!open) return;
    setTab(resolveNavTab(initialTab));
    setSelectImport(initialTab === "import");
  }, [open, initialTab]);

  const isEditing = Boolean(editingModelId);
  const selectedTab = TABS.find((entry) => entry.id === tab) ?? TABS[0];

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
                onClick={() => {
                  setTab(entry.id);
                  setSelectImport(false);
                }}
              >
                <NavIcon className="add-model-modal__nav-icon" />
                <span>{entry.navLabel}</span>
              </button>
              );
            })}
          </nav>

          <div className="add-model-modal__panel">
            <div className="add-model-modal__panel-header">
              <h3 className="add-model-modal__panel-title">{selectedTab.panelTitle}</h3>
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
                  initialSelectImport={selectImport}
                  importForm={{
                    backendConnected,
                    importName,
                    importPath,
                    importBusy,
                    onImportNameChange,
                    onImportPathChange,
                    onBrowseImport,
                    onImport,
                  }}
                />
              </>
            )}

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
