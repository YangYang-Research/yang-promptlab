import { useCallback, useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { ATTACK_MUTATORS } from "@/features/scans/payloadStrategy";
import { Button, IconButton, Modal, PageHeader } from "@/shared/components";
import { IconAi, IconCheck, IconCopy, IconHistory, IconTrash } from "@/shared/components/Icons";
import { toAppError } from "@/shared/errors";
import {
  deletePromptLabHistory,
  expandPromptLab,
  listPromptLabHistory,
  upgradePromptLab,
  type PromptLabHistoryDto,
  type PromptLabVariantDto,
} from "@/shared/ipc/mutators";
import { useToast } from "@/shared/notifications";
import { assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";

function downloadPromptLabJson(payload: unknown, filename: string): void {
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function previewSeed(seed: string): string {
  const oneLine = seed.replace(/\s+/g, " ").trim();
  return oneLine.length > 96 ? `${oneLine.slice(0, 96)}…` : oneLine;
}

function formatHistoryTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString();
}

function VariantCard({
  label,
  mutatorId,
  content,
}: {
  label: string;
  mutatorId: string;
  content: string;
}) {
  const { notify } = useToast();
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      notify(`${label} copied`, "success");
      window.setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      notify(error instanceof Error ? error.message : `Failed to copy ${label}`, "error");
    }
  }

  return (
    <article className="prompt-lab-page__variant">
      <header className="prompt-lab-page__variant-head">
        <div className="prompt-lab-page__variant-titles">
          <h3 className="prompt-lab-page__variant-title">{label}</h3>
          <p className="prompt-lab-page__variant-id">{mutatorId}</p>
        </div>
        <IconButton
          ariaLabel={copied ? `${label} copied` : `Copy ${label}`}
          size="sm"
          active={copied}
          onClick={() => void handleCopy()}
        >
          {copied ? <IconCheck /> : <IconCopy />}
        </IconButton>
      </header>
      <pre className="prompt-lab-page__variant-pre">{content}</pre>
    </article>
  );
}

export function PromptLabPage() {
  const { backendConnected } = useAppStore();
  const { notify } = useToast();
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState<"upgrade" | "expand" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [variants, setVariants] = useState<PromptLabVariantDto[]>([]);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyRows, setHistoryRows] = useState<PromptLabHistoryDto[]>([]);

  const labelById = useMemo(() => {
    const map = new Map<string, string>();
    for (const item of ATTACK_MUTATORS) {
      map.set(item.id, item.label);
    }
    return map;
  }, []);

  async function requireLiveAgent(): Promise<boolean> {
    const yazg = await assertYazgAgentLive(backendConnected);
    if (!yazg.live) {
      setError(yazg.message);
      notify(yazg.message, "error");
      return false;
    }
    return true;
  }

  const loadHistory = useCallback(async () => {
    if (!backendConnected) {
      setHistoryRows([]);
      return;
    }
    setHistoryLoading(true);
    try {
      setHistoryRows(await listPromptLabHistory(50));
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setHistoryLoading(false);
    }
  }, [backendConnected, notify]);

  async function handleOpenHistory() {
    setHistoryOpen(true);
    await loadHistory();
  }

  function handleRestoreHistory(row: PromptLabHistoryDto) {
    setPrompt(row.seedPrompt);
    setVariants(row.variants);
    setHistoryOpen(false);
    setError(null);
    notify("Restored Prompt Lab history", "success");
  }

  async function handleDeleteHistory(id: string) {
    try {
      await deletePromptLabHistory(id);
      setHistoryRows((rows) => rows.filter((row) => row.id !== id));
      notify("History entry deleted", "success");
    } catch (err) {
      notify(toAppError(err).message, "error");
    }
  }

  async function handleUpgrade() {
    const seed = prompt.trim();
    if (!seed) {
      setError("Enter a prompt first.");
      return;
    }
    setBusy("upgrade");
    setError(null);
    try {
      if (!(await requireLiveAgent())) return;
      const upgraded = await upgradePromptLab(seed);
      setPrompt(upgraded.content);
      notify("Prompt upgraded via Attack Factory", "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  async function handleGenerate() {
    const seed = prompt.trim();
    if (!seed) {
      setError("Enter a prompt first.");
      return;
    }
    setBusy("expand");
    setError(null);
    try {
      if (!(await requireLiveAgent())) return;
      const next = await expandPromptLab(seed);
      setVariants(next.variants);
      notify(`Generated ${next.variants.length} variants`, "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      setVariants([]);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  function handleExportJson() {
    const seed = prompt.trim();
    if (!seed || variants.length === 0) {
      setError("Generate variants before exporting.");
      return;
    }
    const stamp = new Date().toISOString().replace(/[:.]/g, "-");
    const payload = {
      exportedAt: new Date().toISOString(),
      seedPrompt: seed,
      variants: variants.map((variant) => ({
        mutatorId: variant.mutatorId,
        label: labelById.get(variant.mutatorId) ?? variant.mutatorId,
        content: variant.content,
      })),
    };
    try {
      downloadPromptLabJson(payload, `prompt-lab-${stamp}.json`);
      notify("Exported Prompt Lab JSON", "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    }
  }

  const actionsDisabled = busy !== null || !prompt.trim();

  return (
    <div className="page prompt-lab-page">
      <PageHeader
        title="Prompt Lab"
        description="An experimental for creating mutator variants from a seed prompt."
      />

      <section className="prompt-lab-page__composer" aria-label="Prompt input">
        <div className="prompt-lab-page__composer-head">
          <label className="prompt-lab-page__label" htmlFor="prompt-lab-input">
            Seed prompt
          </label>
          <IconButton
            ariaLabel="Prompt Lab history"
            size="sm"
            disabled={!backendConnected || busy !== null}
            onClick={() => void handleOpenHistory()}
          >
            <IconHistory />
          </IconButton>
        </div>
        <textarea
          id="prompt-lab-input"
          className="prompt-lab-page__textarea"
          rows={6}
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          placeholder="Paste or type a prompt…"
          disabled={busy !== null}
        />
        <div className="prompt-lab-page__actions">
          <div className="prompt-lab-page__actions-left">
            <Button
              variant="secondary"
              disabled={actionsDisabled}
              onClick={() => void handleUpgrade()}
            >
              <IconAi className="btn__icon" aria-hidden />
              {busy === "upgrade" ? "Upgrading…" : "Upgrade prompt"}
            </Button>
            <Button
              variant="primary"
              disabled={actionsDisabled}
              onClick={() => void handleGenerate()}
            >
              <IconAi className="btn__icon" aria-hidden />
              {busy === "expand" ? "Generating…" : "Generate variants"}
            </Button>
            {variants.length > 0 ? (
              <span className="prompt-lab-page__count">{variants.length} variants</span>
            ) : null}
          </div>
          <Button
            variant="secondary"
            disabled={busy !== null || variants.length === 0}
            onClick={handleExportJson}
          >
            Export Variants
          </Button>
        </div>
        {error ? (
          <p className="prompt-lab-page__error" role="alert">
            {error}
          </p>
        ) : null}
      </section>

      {variants.length > 0 ? (
        <section className="prompt-lab-page__results" aria-label="Mutator variants">
          {variants.map((variant) => (
            <VariantCard
              key={variant.mutatorId}
              mutatorId={variant.mutatorId}
              label={labelById.get(variant.mutatorId) ?? variant.mutatorId}
              content={variant.content}
            />
          ))}
        </section>
      ) : null}

      <Modal
        open={historyOpen}
        title="Prompt Lab history"
        onClose={() => setHistoryOpen(false)}
        size="medium"
      >
        {historyLoading ? (
          <p className="text-muted text-sm">Loading history…</p>
        ) : historyRows.length === 0 ? (
          <p className="text-muted text-sm">No history yet. Generate variants to save an entry.</p>
        ) : (
          <ul className="prompt-lab-page__history-list">
            {historyRows.map((row) => (
              <li key={row.id} className="prompt-lab-page__history-item">
                <button
                  type="button"
                  className="prompt-lab-page__history-main"
                  onClick={() => handleRestoreHistory(row)}
                >
                  <span className="prompt-lab-page__history-time">
                    {formatHistoryTime(row.createdAt)}
                  </span>
                  <span className="prompt-lab-page__history-seed">{previewSeed(row.seedPrompt)}</span>
                  <span className="prompt-lab-page__history-meta">
                    {row.variants.length} variants
                  </span>
                </button>
                <IconButton
                  ariaLabel="Delete history entry"
                  size="sm"
                  variant="danger"
                  onClick={() => void handleDeleteHistory(row.id)}
                >
                  <IconTrash />
                </IconButton>
              </li>
            ))}
          </ul>
        )}
      </Modal>
    </div>
  );
}
