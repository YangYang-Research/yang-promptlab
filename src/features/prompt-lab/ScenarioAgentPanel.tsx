import { useCallback, useMemo, useRef, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  ALL_ATTACK_MUTATOR_IDS,
  ATTACK_MUTATORS,
  type AttackMutatorId,
} from "@/features/scans/payloadStrategy";
import { Button, IconButton, Modal, Select } from "@/shared/components";
import { IconAi, IconCheck, IconCopy, IconHistory, IconTrash } from "@/shared/components/Icons";
import { toAppError } from "@/shared/errors";
import {
  scenarioAgentExpand,
  scenarioAgentFire,
  scenarioAgentFollowup,
  scenarioAgentPlan,
  type ScenarioAgentFireDto,
  type ScenarioAgentVariantDto,
} from "@/shared/ipc/scenarioAgent";
import { useToast } from "@/shared/notifications";
import { assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";

import { AttackChainGraph } from "./AttackChainGraph";
import {
  clearScenarioHistory,
  deleteScenarioHistory,
  loadScenarioHistory,
  newChainNodeId,
  newHistoryId,
  previewText,
  priorTurnsForFollowup,
  saveScenarioHistory,
  upsertScenarioHistory,
  type ScenarioChainNode,
  type ScenarioHistoryEntry,
} from "./scenarioChain";

type AgentMode = "auto" | "manual";

type RunLogEntry = {
  id: string;
  mutatorId: string;
  prompt: string;
  status: number;
  durationMs: number;
  body: string;
  goalHit: boolean;
  matchedKeywords: string[];
};

function CopyableBlock({
  label,
  content,
  meta,
  onCopied,
}: {
  label: string;
  content: string;
  meta?: string;
  /** After successful clipboard copy — e.g. load into Seed prompt. */
  onCopied?: (content: string) => void;
}) {
  const { notify } = useToast();
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(content);
      onCopied?.(content);
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
          {meta ? <p className="prompt-lab-page__variant-id">{meta}</p> : null}
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

export function ScenarioAgentPanel() {
  const { backendConnected, projects, targets } = useAppStore();
  const { notify } = useToast();
  const cancelRef = useRef(false);

  const [scenario, setScenario] = useState("");
  const [goal, setGoal] = useState("");
  const [selectedMutators, setSelectedMutators] = useState<AttackMutatorId[]>([]);
  const [mode, setMode] = useState<AgentMode>("manual");
  const [seed, setSeed] = useState("");
  const [successKeywords, setSuccessKeywords] = useState<string[]>([]);
  const [attackHints, setAttackHints] = useState<string[]>([]);
  const [variants, setVariants] = useState<ScenarioAgentVariantDto[]>([]);
  const [followups, setFollowups] = useState<string[]>([]);
  const [pastedReply, setPastedReply] = useState("");
  const [runLog, setRunLog] = useState<RunLogEntry[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [seedCopied, setSeedCopied] = useState(false);
  const [replyCopied, setReplyCopied] = useState(false);
  const [scenarioCopied, setScenarioCopied] = useState(false);
  const [goalCopied, setGoalCopied] = useState(false);

  // Auto target
  const [targetId, setTargetId] = useState("");
  const [url, setUrl] = useState("");
  const [bodyTemplate, setBodyTemplate] = useState(
    '{"messages":[{"role":"user","content":"{{payload}}"}]}',
  );
  const [maxRounds, setMaxRounds] = useState(3);
  const [maxVariants, setMaxVariants] = useState(5);

  const [chain, setChain] = useState<ScenarioChainNode[]>([]);
  const [activeNodeId, setActiveNodeId] = useState<string | null>(null);
  const [historyId, setHistoryId] = useState<string | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [historyRows, setHistoryRows] = useState<ScenarioHistoryEntry[]>([]);

  const allTargets = useMemo(() => {
    const projectName = new Map(projects.map((p) => [p.id, p.name]));
    return targets.map((t) => ({
      id: t.id,
      name: t.name,
      projectName: projectName.get(t.projectId) ?? t.projectId,
    }));
  }, [projects, targets]);

  const labelById = useMemo(() => {
    const map = new Map<string, string>();
    for (const item of ATTACK_MUTATORS) {
      map.set(item.id, item.label);
    }
    return map;
  }, []);

  function toggleMutator(id: AttackMutatorId) {
    setSelectedMutators((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }

  const allMutatorsSelected =
    selectedMutators.length === ALL_ATTACK_MUTATOR_IDS.length &&
    ALL_ATTACK_MUTATOR_IDS.every((id) => selectedMutators.includes(id));

  function selectAllMutators() {
    setSelectedMutators([...ALL_ATTACK_MUTATOR_IDS]);
  }

  function clearMutators() {
    setSelectedMutators([]);
  }

  const persistChain = useCallback(
    (nodes: ScenarioChainNode[], histId: string | null) => {
      if (nodes.length === 0) return histId;
      const id = histId ?? newHistoryId();
      const entry: ScenarioHistoryEntry = {
        id,
        createdAt: new Date().toISOString(),
        scenario: scenario.trim(),
        goal: goal.trim(),
        nodes,
      };
      const next = upsertScenarioHistory(entry, loadScenarioHistory());
      saveScenarioHistory(next);
      setHistoryId(id);
      return id;
    },
    [scenario, goal],
  );

  function selectChainNode(node: ScenarioChainNode) {
    setActiveNodeId(node.id);
    setSeed(node.seed);
    setPastedReply(node.reply ?? "");
  }

  function openHistory() {
    setHistoryRows(loadScenarioHistory());
    setHistoryOpen(true);
  }

  function restoreHistory(entry: ScenarioHistoryEntry) {
    setScenario(entry.scenario);
    setGoal(entry.goal);
    setChain(entry.nodes);
    setHistoryId(entry.id);
    // Prefer a leaf (no children); else last node.
    const leaf =
      [...entry.nodes].reverse().find((n) => !entry.nodes.some((c) => c.parentId === n.id)) ??
      entry.nodes[entry.nodes.length - 1];
    if (leaf) {
      setActiveNodeId(leaf.id);
      setSeed(leaf.seed);
      setPastedReply(leaf.reply ?? "");
    }
    setHistoryOpen(false);
    notify("Restored Scenario history", "success");
  }

  function removeHistory(id: string) {
    const next = deleteScenarioHistory(id, loadScenarioHistory());
    saveScenarioHistory(next);
    setHistoryRows(next);
    if (historyId === id) setHistoryId(null);
    notify("Scenario history entry deleted", "success");
  }

  async function requireLiveAgent(): Promise<boolean> {
    const yazg = await assertYazgAgentLive(backendConnected);
    if (!yazg.live) {
      setError(yazg.message);
      notify(yazg.message, "error");
      return false;
    }
    return true;
  }

  async function handlePlan() {
    if (!(await requireLiveAgent())) return;
    setBusy("plan");
    setError(null);
    try {
      const plan = await scenarioAgentPlan({
        scenario,
        goal,
        mutatorHint: selectedMutators,
        successKeywordHint: successKeywords,
      });
      setSeed(plan.seedPrompt);
      if (plan.recommendedMutators.length > 0) {
        setSelectedMutators(
          plan.recommendedMutators.filter((id): id is AttackMutatorId =>
            ATTACK_MUTATORS.some((m) => m.id === id),
          ),
        );
      }
      if (plan.successKeywords.length > 0) {
        setSuccessKeywords(plan.successKeywords);
      }
      setAttackHints(plan.attackHints);
      const node: ScenarioChainNode = {
        id: newChainNodeId(),
        parentId: null,
        seed: plan.seedPrompt,
        reply: null,
      };
      const nodes = [node];
      setChain(nodes);
      setActiveNodeId(node.id);
      setPastedReply("");
      setFollowups([]);
      persistChain(nodes, null);
      notify("Seed planned", "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  async function handleExpand() {
    const prompt = seed.trim();
    if (!prompt) {
      setError("Plan a seed prompt first.");
      return;
    }
    if (selectedMutators.length === 0) {
      setError("Select at least one mutator.");
      return;
    }
    const needsLlm = selectedMutators.some((id) => id.startsWith("llm_"));
    if (needsLlm && !(await requireLiveAgent())) return;

    setBusy("expand");
    setError(null);
    try {
      const result = await scenarioAgentExpand(prompt, selectedMutators);
      setVariants(result.variants);
      notify(`${result.variants.length} variants`, "success");
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  async function requestFollowups(
    lastPrompt: string,
    lastResponse: string,
    extraTurns: { seed: string; reply: string }[] = [],
  ): Promise<string[]> {
    if (!(await requireLiveAgent())) return [];
    const result = await scenarioAgentFollowup({
      scenario,
      goal,
      lastPrompt,
      lastResponse,
      transcript: priorTurnsForFollowup(chain, lastPrompt, lastResponse, extraTurns),
      successKeywords,
    });
    setFollowups(result.prompts);

    const nextSeed = result.prompts[0]?.trim() ?? "";
    setChain((prev) => {
      // Branch from the *active* node — never append after an unrelated leaf.
      let nodes =
        prev.length > 0
          ? [...prev]
          : [
              {
                id: newChainNodeId(),
                parentId: null,
                seed: lastPrompt,
                reply: null,
              },
            ];
      const found = nodes.findIndex((n) => n.id === activeNodeId);
      const idx = found >= 0 ? found : nodes.length - 1;
      const parent = nodes[idx];
      nodes[idx] = {
        ...parent,
        seed: parent.seed || lastPrompt,
        reply: lastResponse,
      };
      if (nextSeed) {
        const nextNode: ScenarioChainNode = {
          id: newChainNodeId(),
          parentId: parent.id,
          seed: nextSeed,
          reply: null,
        };
        nodes = [...nodes, nextNode];
        setActiveNodeId(nextNode.id);
        setSeed(nextSeed);
        setPastedReply("");
      }
      persistChain(nodes, historyId);
      return nodes;
    });

    notify(result.rationale || "Follow-ups ready", "success");
    return result.prompts;
  }

  async function handleFollowup(
    lastPrompt: string,
    lastResponse: string,
  ): Promise<string[]> {
    setBusy("followup");
    setError(null);
    try {
      return await requestFollowups(lastPrompt, lastResponse);
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
      return [];
    } finally {
      setBusy(null);
    }
  }

  const fireOne = useCallback(
    async (payload: string): Promise<ScenarioAgentFireDto> => {
      return scenarioAgentFire({
        url: url.trim() || null,
        targetId: targetId.trim() || null,
        bodyTemplate: bodyTemplate.trim() || null,
        method: "POST",
        payload,
        timeoutMs: 60_000,
        successKeywords,
      });
    },
    [url, targetId, bodyTemplate, successKeywords],
  );

  async function handleAutoRun() {
    if (variants.length === 0) {
      setError("Expand variants before Auto run.");
      return;
    }
    if (!targetId.trim() && !url.trim()) {
      setError("Auto mode needs a target URL or saved target.");
      return;
    }
    cancelRef.current = false;
    setBusy("auto");
    setError(null);
    setRunLog([]);

    let queue = variants.slice(0, Math.max(1, maxVariants));
    let round = 0;
    let hit = false;
    const fired: { seed: string; reply: string }[] = [];

    try {
      while (round < maxRounds && !hit && !cancelRef.current) {
        round += 1;
        let lastPrompt = "";
        let lastBody = "";
        for (const variant of queue) {
          if (cancelRef.current) break;
          const result = await fireOne(variant.content);
          lastPrompt = variant.content;
          lastBody = result.body;
          fired.push({ seed: variant.content, reply: result.body });
          const entry: RunLogEntry = {
            id: `${Date.now()}-${variant.mutatorId}`,
            mutatorId: variant.mutatorId,
            prompt: variant.content,
            status: result.status,
            durationMs: result.durationMs,
            body: result.body,
            goalHit: result.goalHit,
            matchedKeywords: result.matchedKeywords,
          };
          setRunLog((prev) => [entry, ...prev]);
          if (result.goalHit) {
            hit = true;
            notify("Goal hit", "success");
            break;
          }
        }
        if (hit || cancelRef.current || round >= maxRounds) break;
        if (!(await requireLiveAgent())) break;
        const nextPrompts = await requestFollowups(lastPrompt, lastBody, fired);
        if (nextPrompts.length === 0) break;
        queue = nextPrompts.map((content, index) => ({
          mutatorId: `followup_${index + 1}`,
          content,
        }));
        setVariants(queue);
        setSeed(nextPrompts[0] ?? seed);
      }
      if (!hit && !cancelRef.current) {
        notify("Auto run finished (budget exhausted)", "info");
      }
    } catch (err) {
      const message = toAppError(err).message;
      setError(message);
      notify(message, "error");
    } finally {
      setBusy(null);
    }
  }

  function handleStopAuto() {
    cancelRef.current = true;
  }

  /** Copy a variant / follow-up option into the Seed prompt field (and active chain node). */
  function applyCopiedSeed(content: string) {
    const next = content.trim();
    if (!next) return;
    setSeed(next);
    if (!activeNodeId) return;
    setChain((prev) =>
      prev.map((n) => (n.id === activeNodeId ? { ...n, seed: next } : n)),
    );
  }

  function handleClearWorkspace() {
    if (busy) return;
    const hasContent = Boolean(
      scenario.trim() ||
        goal.trim() ||
        seed.trim() ||
        pastedReply.trim() ||
        selectedMutators.length ||
        variants.length ||
        followups.length ||
        chain.length ||
        runLog.length ||
        error,
    );
    if (!hasContent) return;
    if (!window.confirm("Clear scenario workspace (seed, chain, variants, mutators, reply)?")) return;
    cancelRef.current = true;
    setScenario("");
    setGoal("");
    setSeed("");
    setPastedReply("");
    setSelectedMutators([]);
    setSuccessKeywords([]);
    setAttackHints([]);
    setVariants([]);
    setFollowups([]);
    setRunLog([]);
    setChain([]);
    setActiveNodeId(null);
    setHistoryId(null);
    setError(null);
    notify("Scenario cleared", "success");
  }

  function handleClearAllHistory() {
    if (historyRows.length === 0) return;
    if (!window.confirm(`Clear all ${historyRows.length} Scenario history entries?`)) return;
    clearScenarioHistory();
    setHistoryRows([]);
    notify("Scenario history cleared", "success");
  }

  const actionsDisabled = busy !== null;

  async function copyField(
    text: string,
    label: string,
    markCopied: (copied: boolean) => void,
  ) {
    const value = text.trim();
    if (!value) {
      notify(`${label} is empty.`, "error");
      return;
    }
    try {
      await navigator.clipboard.writeText(value);
      markCopied(true);
      notify(`${label} copied`, "success");
      window.setTimeout(() => markCopied(false), 1600);
    } catch (err) {
      notify(err instanceof Error ? err.message : `Failed to copy ${label}`, "error");
    }
  }
  const clearDisabled =
    actionsDisabled ||
    !(
      scenario.trim() ||
      goal.trim() ||
      seed.trim() ||
      pastedReply.trim() ||
      selectedMutators.length ||
      variants.length ||
      followups.length ||
      chain.length ||
      runLog.length ||
      error
    );

  return (
    <div className="scenario-agent">
      <section className="prompt-lab-page__composer" aria-label="Scenario">
        <div className="prompt-lab-page__composer-head">
          <span className="prompt-lab-page__label">Scenario</span>
          <IconButton
            ariaLabel="Scenario history"
            size="sm"
            disabled={actionsDisabled}
            onClick={openHistory}
          >
            <IconHistory />
          </IconButton>
        </div>

        <div className="field" role="group" aria-label="Mode">
          <span className="field__label">Mode</span>
          <div className="wizard-attack-profiles">
            <button
              type="button"
              className={`wizard-attack-profile${mode === "manual" ? " wizard-attack-profile--selected" : ""}`}
              disabled={actionsDisabled}
              aria-pressed={mode === "manual"}
              onClick={() => setMode("manual")}
            >
              <span className="wizard-attack-profile__label">Manual</span>
              <span className="wizard-attack-profile__description text-sm">
                Copy probes, paste target replies, get follow-ups.
              </span>
            </button>
            <button
              type="button"
              className="wizard-attack-profile wizard-attack-profile--recommended"
              disabled
              aria-disabled="true"
              title="Coming Soon"
            >
              <span className="wizard-attack-profile__yazg-badge">Coming Soon</span>
              <span className="wizard-attack-profile__label">Auto</span>
              <span className="wizard-attack-profile__description text-sm">
                POST variants through harness, adapt until goal or budget.
              </span>
            </button>
          </div>
        </div>

        <div className="field">
          <div className="prompt-lab-page__composer-head">
            <label className="field__label" htmlFor="scenario-text">
              Scenario
            </label>
            <IconButton
              ariaLabel={scenarioCopied ? "Scenario copied" : "Copy scenario"}
              size="sm"
              active={scenarioCopied}
              disabled={!scenario.trim()}
              onClick={() => void copyField(scenario, "Scenario", setScenarioCopied)}
            >
              {scenarioCopied ? <IconCheck /> : <IconCopy />}
            </IconButton>
          </div>
          <textarea
            id="scenario-text"
            className="input textarea"
            rows={4}
            value={scenario}
            disabled={actionsDisabled}
            placeholder="Describe the target assistant, tools, trust boundary, and any constraints…"
            onChange={(e) => setScenario(e.target.value)}
          />
        </div>

        <div className="field">
          <div className="prompt-lab-page__composer-head">
            <label className="field__label" htmlFor="scenario-goal">
              Goal
            </label>
            <IconButton
              ariaLabel={goalCopied ? "Goal copied" : "Copy goal"}
              size="sm"
              active={goalCopied}
              disabled={!goal.trim()}
              onClick={() => void copyField(goal, "Goal", setGoalCopied)}
            >
              {goalCopied ? <IconCheck /> : <IconCopy />}
            </IconButton>
          </div>
          <textarea
            id="scenario-goal"
            className="input textarea"
            rows={2}
            value={goal}
            disabled={actionsDisabled}
            placeholder="What should the probe achieve?"
            onChange={(e) => setGoal(e.target.value)}
          />
        </div>

        {successKeywords.length > 0 ? (
          <div className="scenario-agent__keywords" aria-label="Success keywords">
            <span className="field__label">Success keywords (from plan)</span>
            <div className="scenario-agent__keyword-chips">
              {successKeywords.map((kw) => (
                <span key={kw} className="scenario-agent__meta-pill">
                  {kw}
                </span>
              ))}
            </div>
          </div>
        ) : null}

        {mode === "auto" ? (
          <div className="scenario-agent__auto-target">
            <label className="field">
              <span className="field__label">Saved target (optional)</span>
              <Select
                id="scenario-target"
                value={targetId}
                disabled={actionsDisabled}
                onChange={(e) => setTargetId(e.target.value)}
              >
                <option value="">Use inline URL…</option>
                {allTargets.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.projectName} / {t.name}
                  </option>
                ))}
              </Select>
            </label>
            <label className="field">
              <span className="field__label">POST URL</span>
              <input
                id="scenario-url"
                className="input"
                value={url}
                disabled={actionsDisabled || Boolean(targetId)}
                placeholder="https://example.com/v1/chat/completions"
                onChange={(e) => setUrl(e.target.value)}
              />
            </label>
            <label className="field">
              <span className="field__label">Body template (`{"{{payload}}"}`)</span>
              <textarea
                id="scenario-body"
                className="input textarea"
                rows={3}
                value={bodyTemplate}
                disabled={actionsDisabled}
                onChange={(e) => setBodyTemplate(e.target.value)}
              />
            </label>
            <div className="scenario-agent__budget">
              <label>
                Max variants
                <input
                  type="number"
                  min={1}
                  max={20}
                  value={maxVariants}
                  disabled={actionsDisabled}
                  onChange={(e) => setMaxVariants(Number(e.target.value) || 1)}
                />
              </label>
              <label>
                Max rounds
                <input
                  type="number"
                  min={1}
                  max={10}
                  value={maxRounds}
                  disabled={actionsDisabled}
                  onChange={(e) => setMaxRounds(Number(e.target.value) || 1)}
                />
              </label>
            </div>
          </div>
        ) : null}

        <div className="scenario-agent__mutators" aria-label="Mutators">
          <p className="field__label">
            Mutators ({selectedMutators.length}/{ALL_ATTACK_MUTATOR_IDS.length} — Plan ordered try-queue; Expand one wrap at a time)
          </p>
          <div className="scenario-agent__mutator-chips">
            <button
              type="button"
              className={
                allMutatorsSelected ? "filter-chip filter-chip--active" : "filter-chip"
              }
              disabled={actionsDisabled}
              onClick={() => (allMutatorsSelected ? clearMutators() : selectAllMutators())}
              title={allMutatorsSelected ? "Clear all mutators" : "Select all mutators"}
            >
              All
            </button>
            {ATTACK_MUTATORS.map((m) => {
              const active = selectedMutators.includes(m.id);
              return (
                <button
                  key={m.id}
                  type="button"
                  className={active ? "filter-chip filter-chip--active" : "filter-chip"}
                  disabled={actionsDisabled}
                  onClick={() => toggleMutator(m.id)}
                >
                  {m.label}
                </button>
              );
            })}
          </div>
        </div>

        <div className="prompt-lab-page__actions">
          <div className="prompt-lab-page__actions-left">
            <Button
              variant="secondary"
              disabled={actionsDisabled || !scenario.trim() || !goal.trim()}
              onClick={() => void handlePlan()}
            >
              <IconAi
                className={["btn__icon", busy === "plan" ? "btn__icon--working" : ""]
                  .filter(Boolean)
                  .join(" ")}
                aria-hidden
              />
              {busy === "plan" ? "Planning…" : "Plan seed"}
            </Button>
            <Button
              variant="primary"
              disabled={actionsDisabled || !seed.trim()}
              onClick={() => void handleExpand()}
            >
              <IconAi
                className={["btn__icon", busy === "expand" ? "btn__icon--working" : ""]
                  .filter(Boolean)
                  .join(" ")}
                aria-hidden
              />
              {busy === "expand" ? "Expanding…" : "Expand mutators"}
            </Button>
            {mode === "auto" ? (
              busy === "auto" ? (
                <Button variant="secondary" onClick={handleStopAuto}>
                  Stop
                </Button>
              ) : (
                <Button
                  variant="primary"
                  disabled={actionsDisabled || variants.length === 0}
                  onClick={() => void handleAutoRun()}
                >
                  Auto run
                </Button>
              )
            ) : null}
          </div>
          <Button variant="secondary" disabled={clearDisabled} onClick={handleClearWorkspace}>
            Clear
          </Button>
        </div>

        {error ? (
          <p className="prompt-lab-page__error" role="alert">
            {error}
          </p>
        ) : null}

        {attackHints.length > 0 ? (
          <ul className="scenario-agent__hints">
            {attackHints.map((hint) => (
              <li key={hint}>{hint}</li>
            ))}
          </ul>
        ) : null}
      </section>

      {chain.length > 0 ? (
        <section className="scenario-chain" aria-label="Graph Chain">
          <div className="scenario-chain__head">
            <h3 className="scenario-chain__title">Graph Chain</h3>
            <span className="scenario-chain__meta">{chain.length} seeds</span>
          </div>
          <AttackChainGraph
            chain={chain}
            activeNodeId={activeNodeId}
            onSelect={selectChainNode}
          />
        </section>
      ) : null}

      <section className="scenario-agent__io-row" aria-label="Seed and target reply">
        <div className="scenario-agent__io-col">
          <div className="prompt-lab-page__composer-head">
            <label className="prompt-lab-page__label" htmlFor="scenario-seed">
              Seed prompt
            </label>
            <IconButton
              ariaLabel={seedCopied ? "Seed prompt copied" : "Copy seed prompt"}
              size="sm"
              active={seedCopied}
              disabled={!seed.trim()}
              onClick={() => void copyField(seed, "Seed prompt", setSeedCopied)}
            >
              {seedCopied ? <IconCheck /> : <IconCopy />}
            </IconButton>
          </div>
          <textarea
            id="scenario-seed"
            className="prompt-lab-page__textarea"
            rows={6}
            value={seed}
            disabled={actionsDisabled}
            placeholder="Plan a seed, or click a chain node to restore…"
            onChange={(e) => {
              const value = e.target.value;
              setSeed(value);
              if (!activeNodeId) return;
              setChain((prev) =>
                prev.map((n) => (n.id === activeNodeId ? { ...n, seed: value } : n)),
              );
            }}
          />
        </div>
        <div className="scenario-agent__io-col">
          <div className="prompt-lab-page__composer-head">
            <label className="prompt-lab-page__label" htmlFor="scenario-reply">
              Paste target reply
            </label>
            <IconButton
              ariaLabel={replyCopied ? "Target reply copied" : "Copy target reply"}
              size="sm"
              active={replyCopied}
              disabled={!pastedReply.trim()}
              onClick={() => void copyField(pastedReply, "Target reply", setReplyCopied)}
            >
              {replyCopied ? <IconCheck /> : <IconCopy />}
            </IconButton>
          </div>
          <textarea
            id="scenario-reply"
            className="prompt-lab-page__textarea"
            rows={6}
            value={pastedReply}
            disabled={actionsDisabled}
            onChange={(e) => {
              const value = e.target.value;
              setPastedReply(value);
              if (!activeNodeId) return;
              setChain((prev) =>
                prev.map((n) =>
                  n.id === activeNodeId ? { ...n, reply: value.trim() ? value : null } : n,
                ),
              );
            }}
            placeholder="Paste the assistant response here…"
          />
          {mode === "manual" ? (
            <Button
              variant="secondary"
              disabled={
                actionsDisabled || !pastedReply.trim() || !(seed.trim() || variants[0]?.content)
              }
              onClick={() =>
                void handleFollowup(variants[0]?.content ?? seed, pastedReply)
              }
            >
              {busy === "followup" ? "Thinking…" : "Follow-up prompts"}
            </Button>
          ) : null}
        </div>
      </section>

      {followups.length > 0 ? (
        <section className="prompt-lab-page__results" aria-label="Follow-ups">
          {followups.map((prompt, index) => (
            <CopyableBlock
              key={`fu-${index}`}
              label={`Follow-up option #${index + 1}`}
              content={prompt}
              onCopied={applyCopiedSeed}
            />
          ))}
        </section>
      ) : null}

      {variants.length > 0 ? (
        <section className="prompt-lab-page__results" aria-label="Variants">
          {variants.map((variant) => (
            <CopyableBlock
              key={variant.mutatorId}
              label={labelById.get(variant.mutatorId) ?? variant.mutatorId}
              meta={variant.mutatorId}
              content={variant.content}
              onCopied={applyCopiedSeed}
            />
          ))}
        </section>
      ) : null}

      {runLog.length > 0 ? (
        <section className="scenario-agent__log" aria-label="Auto run log">
          <h3 className="scenario-agent__log-title">Run log</h3>
          {runLog.map((entry) => (
            <article
              key={entry.id}
              className={
                entry.goalHit
                  ? "scenario-agent__log-item scenario-agent__log-item--hit"
                  : "scenario-agent__log-item"
              }
            >
              <header>
                <strong>{entry.mutatorId}</strong>
                <span>
                  HTTP {entry.status} · {entry.durationMs}ms
                  {entry.goalHit ? " · GOAL HIT" : ""}
                </span>
              </header>
              {entry.matchedKeywords.length > 0 ? (
                <p className="scenario-agent__hint">
                  Matched: {entry.matchedKeywords.join(", ")}
                </p>
              ) : null}
              <pre className="prompt-lab-page__variant-pre">{entry.body}</pre>
            </article>
          ))}
        </section>
      ) : null}

      <Modal
        open={historyOpen}
        title="Scenario history"
        onClose={() => setHistoryOpen(false)}
        size="medium"
        footer={
          historyRows.length > 0 ? (
            <Button variant="danger" onClick={handleClearAllHistory}>
              Clear all
            </Button>
          ) : null
        }
      >
        {historyRows.length === 0 ? (
          <p className="text-muted text-sm">
            No Scenario history yet. Plan a seed to save a chain.
          </p>
        ) : (
          <ul className="prompt-lab-page__history-list">
            {historyRows.map((row) => (
              <li key={row.id} className="prompt-lab-page__history-item">
                <button
                  type="button"
                  className="prompt-lab-page__history-main"
                  onClick={() => restoreHistory(row)}
                >
                  <span className="prompt-lab-page__history-time">
                    {new Date(row.createdAt).toLocaleString()}
                  </span>
                  <span className="prompt-lab-page__history-seed">
                    {previewText(row.goal || row.scenario || row.nodes[0]?.seed || "(empty)")}
                  </span>
                  <span className="prompt-lab-page__history-meta">
                    {row.nodes.length} seed{row.nodes.length === 1 ? "" : "s"}
                  </span>
                </button>
                <IconButton
                  ariaLabel="Delete Scenario history entry"
                  size="sm"
                  variant="danger"
                  onClick={() => removeHistory(row.id)}
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
