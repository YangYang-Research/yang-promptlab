import { useEffect, useState } from "react";

import { Modal } from "@/shared/components";

import { ThirdPartyModelsPanel } from "./ThirdPartyModelsPanel";
import type { ThirdPartyModelForm } from "@/shared/ipc/thirdPartyModels";

export type AddModelTab = "third-party";

type AddModelModalProps = {
  open: boolean;
  onClose: () => void;
  initialTab?: AddModelTab;
  initialThirdPartyForm?: ThirdPartyModelForm | null;
  editingModelId?: string | null;
  modalTitle?: string;
  backendConnected: boolean;
  onThirdPartySaved: () => void;
};

export function AddModelModal({
  open,
  onClose,
  backendConnected,
  onThirdPartySaved,
  initialThirdPartyForm = null,
  editingModelId = null,
  modalTitle = "Add Model",
}: AddModelModalProps) {
  const [mounted, setMounted] = useState(open);

  useEffect(() => {
    if (open) setMounted(true);
  }, [open]);

  const isEditing = Boolean(editingModelId);

  return (
    <Modal open={open} title={modalTitle} onClose={onClose} size="medium">
      {mounted && (
        <ThirdPartyModelsPanel
          key={
            open
              ? isEditing
                ? `edit-${editingModelId}`
                : `third-party-panel-${initialThirdPartyForm?.provider ?? "new"}-${initialThirdPartyForm?.model ?? ""}`
              : "closed"
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
    </Modal>
  );
}
