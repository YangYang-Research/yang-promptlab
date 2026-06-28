import { useEffect, useState } from "react";

import { Button, Modal } from "@/shared/components";

import { curlToProfilePatch } from "../curlImport";
import type { TargetProfileFormState } from "../targetProfile";

const EXAMPLE_CURL = `curl -X POST 'https://api.openai.com/v1/chat/completions' \\
  -H 'Content-Type: application/json' \\
  -H 'Authorization: Bearer YOUR_API_KEY' \\
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{ "role": "user", "content": "Hello" }]
  }'`;

type ImportApiModalProps = {
  open: boolean;
  onClose: () => void;
  onImport: (patch: Partial<TargetProfileFormState>) => void;
};

export function ImportApiModal({ open, onClose, onImport }: ImportApiModalProps) {
  const [curlText, setCurlText] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setCurlText("");
    setError(null);
  }, [open]);

  function handleClose() {
    onClose();
  }

  function handleImport() {
    const result = curlToProfilePatch(curlText);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    onImport(result.patch);
    onClose();
  }

  return (
    <Modal
      open={open}
      title="Import your API"
      size="medium"
      onClose={handleClose}
      footer={
        <div className="project-form__actions">
          <Button variant="ghost" onClick={handleClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={handleImport} disabled={!curlText.trim()}>
            Import
          </Button>
        </div>
      }
    >
      <div className="project-form import-api-modal">
        <p className="text-muted text-sm import-api-modal__lead">
          Paste a cURL command from your browser or API client. AISec will fill endpoint, method,
          headers, and body template automatically.
        </p>
        <label className="field">
          <span className="field__label">cURL command</span>
          <textarea
            className="input textarea import-api-modal__curl wizard-target-form__mono"
            rows={12}
            value={curlText}
            onChange={(e) => {
              setError(null);
              setCurlText(e.target.value);
            }}
            placeholder={EXAMPLE_CURL}
            spellCheck={false}
            autoFocus
          />
        </label>
        {error && <p className="text-danger">{error}</p>}
      </div>
    </Modal>
  );
}
