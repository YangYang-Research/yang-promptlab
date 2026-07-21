import { useEffect, useState } from "react";

import { Button, Modal, Select } from "@/shared/components";
import type { Finding } from "@/shared/types";

const STATUS_OPTIONS: Array<{ value: Finding["status"]; label: string }> = [
  { value: "open", label: "Open" },
  { value: "confirmed", label: "Confirmed" },
  { value: "false_positive", label: "False positive" },
  { value: "fixed", label: "Fixed" },
];

type UpdateFindingStatusModalProps = {
  open: boolean;
  currentStatus: Finding["status"];
  submitting?: boolean;
  onClose: () => void;
  onSubmit: (status: Finding["status"]) => void | Promise<void>;
};

export function UpdateFindingStatusModal({
  open,
  currentStatus,
  submitting = false,
  onClose,
  onSubmit,
}: UpdateFindingStatusModalProps) {
  const [status, setStatus] = useState<Finding["status"]>(currentStatus);

  useEffect(() => {
    if (!open) return;
    setStatus(currentStatus);
  }, [open, currentStatus]);

  return (
    <Modal
      open={open}
      title="Update status"
      onClose={() => {
        if (!submitting) onClose();
      }}
      footer={
        <div className="project-form__actions">
          <Button variant="ghost" disabled={submitting} onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={submitting || status === currentStatus}
            onClick={() => void onSubmit(status)}
          >
            {submitting ? "Saving…" : "Save"}
          </Button>
        </div>
      }
    >
      <div className="field">
        <label className="field__label" htmlFor="finding-status">
          Status
        </label>
        <Select
          id="finding-status"
          value={status}
          onChange={(event) => setStatus(event.target.value as Finding["status"])}
          disabled={submitting}
        >
          {STATUS_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </Select>
      </div>
    </Modal>
  );
}
