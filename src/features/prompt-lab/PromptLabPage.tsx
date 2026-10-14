import { useCallback, useMemo, useState, useSyncExternalStore } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { ATTACK_MUTATORS } from "@/features/scans/payloadStrategy";
import { Button, IconButton, Modal, PageHeader, Skeleton } from "@/shared/components";
import { IconAi, IconCheck, IconCopy, IconHistory, IconTrash } from "@/shared/components/Icons";
import { toAppError } from "@/shared/errors";
import {
  deletePromptLabHistory,
  expandPromptLab,
  listPromptLabHistory,
  upgradePromptLab,
  type PromptLabHistoryDto,
} from "@/shared/ipc/mutators";
import { useToast } from "@/shared/notifications";
import {
  getPromptLabUiState,
  patchPromptLabUiState,
  subscribePromptLabUiState,
} from "@/shared/runtime/promptLabUiState";
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
  const ui = useSyncExternalStore(
    subscribePromptLabUiState,
    getPromptLabUiState,
    getPromptLabUiState,
  );
  const { prompt, busy, error, variants } = ui;
  const [historyOpen, setHistoryOpen] = useState(false);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyRows, setHistoryRows] = useState<PromptLabHistoryDto[]>([]);
  const [seedCopied, setSeedCopied] = useState(false);

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
      patchPromptLabUiState({ error: yazg.message, busy: null });
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

  async function handleCopySeed() {
    const seed = prompt.trim();
    if (!seed) {
      notify("Enter a prompt first.", "error");
      return;
    }
    try {
      await navigator.clipboard.writeText(seed);
      setSeedCopied(true);
      notify("Seed prompt copied", "success");
      window.setTimeout(() => setSeedCopied(false), 1600);
    } catch (error) {
      notify(error instanceof Error ? error.message : "Failed to copy seed prompt", "error");
    }
  }

  function handleRestoreHistory(row: PromptLabHistoryDto) {
    patchPromptLabUiState({
      prompt: row.seedPrompt,
      variants: row.variants,
      error: null,
    });
    setHistoryOpen(false);
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
      patchPromptLabUiState({ error: "Enter a prompt first." });
      return;
    }
    if (busy) return;
    patchPromptLabUiState({ busy: "upgrade", error: null });
    try {
      if (!(await requireLiveAgent())) return;
      const upgraded = await upgradePromptLab(seed);
      patchPromptLabUiState({ prompt: upgraded.content });
      notify("Prompt upgraded via Attack Factory", "success");
    } catch (err) {
      const message = toAppError(err).message;
      patchPromptLabUiState({ error: message });
      notify(message, "error");
    } finally {
      patchPromptLabUiState({ busy: null });
    }
  }

  async function handleGenerate() {
    const seed = prompt.trim();
    if (!seed) {
      patchPromptLabUiState({ error: "Enter a prompt first." });
      return;
    }
    if (busy) return;
    patchPromptLabUiState({ busy: "expand", error: null });
    try {
      if (!(await requireLiveAgent())) return;
      const next = await expandPromptLab(seed);
      patchPromptLabUiState({ variants: next.variants });
      notify(`Generated ${next.variants.length} variants`, "success");
    } catch (err) {
      const message = toAppError(err).message;
      patchPromptLabUiState({ error: message, variants: [] });
      notify(message, "error");
    } finally {
      patchPromptLabUiState({ busy: null });
    }
  }

  function handleExportJson() {
    const seed = prompt.trim();
    if (!seed || variants.length === 0) {
      patchPromptLabUiState({ error: "Generate variants before exporting." });
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
      patchPromptLabUiState({ error: message });
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
          <div className="prompt-lab-page__composer-tools">
            <IconButton
              ariaLabel={seedCopied ? "Seed prompt copied" : "Copy seed prompt"}
              size="sm"
              active={seedCopied}
              disabled={!prompt.trim() || busy !== null}
              onClick={() => void handleCopySeed()}
            >
              {seedCopied ? <IconCheck /> : <IconCopy />}
            </IconButton>
            <IconButton
              ariaLabel="Prompt Lab history"
              size="sm"
              disabled={!backendConnected || busy !== null}
              onClick={() => void handleOpenHistory()}
            >
              <IconHistory />
            </IconButton>
          </div>
        </div>
        <textarea
          id="prompt-lab-input"
          className="prompt-lab-page__textarea"
          rows={6}
          value={prompt}
          onChange={(event) => patchPromptLabUiState({ prompt: event.target.value })}
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
              <IconAi
                className={["btn__icon", busy === "upgrade" ? "btn__icon--working" : ""]
                  .filter(Boolean)
                  .join(" ")}
                aria-hidden
              />
              {busy === "upgrade" ? "Upgrading…" : "Upgrade prompt"}
            </Button>
            <Button
              variant="primary"
              disabled={actionsDisabled}
              onClick={() => void handleGenerate()}
            >
              <IconAi
                className={["btn__icon", busy === "expand" ? "btn__icon--working" : ""]
                  .filter(Boolean)
                  .join(" ")}
                aria-hidden
              />
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

      {busy === "expand" ? (
        <section
          className="prompt-lab-page__loading"
          aria-busy="true"
          aria-live="polite"
          aria-label="Generating variants"
        >
          <div className="prompt-lab-page__loading-head">
            <span className="page-loader__spinner" aria-hidden />
            <p className="prompt-lab-page__loading-title">
              <IconAi className="prompt-lab-page__loading-icon" aria-hidden />
              Generating
            </p>
          </div>
          <div className="prompt-lab-page__results" aria-hidden>
            {Array.from({ length: 6 }).map((_, index) => (
              <div key={index} className="prompt-lab-page__variant prompt-lab-page__variant--skeleton">
                <Skeleton width="42%" height="0.85rem" />
                <Skeleton width="28%" height="0.65rem" />
                <Skeleton width="100%" height="4.5rem" />
              </div>
            ))}
          </div>
        </section>
      ) : variants.length > 0 ? (
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
