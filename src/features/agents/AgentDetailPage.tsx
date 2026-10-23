import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Card, EmptyState, IconButton, PageHeader } from "@/shared/components";
import { IconCheck, IconCopy } from "@/shared/components/Icons";
import { toAppError } from "@/shared/errors";
import {
  getAgent,
  setAgentAttempt,
  setAgentTemperature,
  type AgentDetailDto,
} from "@/shared/ipc/agents";
import { useToast } from "@/shared/notifications";

const ATTEMPT_MIN = 1;
const ATTEMPT_MAX = 10;
const ATTEMPT_DEFAULT = 3;
const TEMP_MIN = 0;
const TEMP_MAX = 2;
const TEMP_STEP = 0.1;
const AUTO_SAVE_DELAY_MS = 500;

function fillPercent(value: number, min: number, max: number): number {
  if (max <= min) return 0;
  return ((value - min) / (max - min)) * 100;
}

function clampAttempt(value: number): number {
  return Math.min(ATTEMPT_MAX, Math.max(ATTEMPT_MIN, Math.round(value)));
}

function roundTemperature(value: number): number {
  return Math.round(Math.min(TEMP_MAX, Math.max(TEMP_MIN, value)) * 10) / 10;
}

function sameTemperature(left: number | null, right: number | null): boolean {
  if (left == null || right == null) return left === right;
  return Math.round(left * 10) === Math.round(right * 10);
}

function MetricSlider({
  label,
  hint,
  value,
  min,
  max,
  step,
  display,
  disabled,
  onChange,
}: {
  label: string;
  hint: string;
  value: number;
  min: number;
  max: number;
  step: number;
  display: string;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <div className="settings-judge-weight">
      <div className="settings-judge-weight__header">
        <div>
          <span className="settings-judge-weight__label">{label}</span>
          <span className="settings-judge-weight__hint">{hint}</span>
        </div>
        <span className="settings-judge-weight__value mono">{display}</span>
      </div>
      <div className="settings-judge-weight__track-wrap">
        <div className="progress__track" aria-hidden>
          <div
            className="progress__fill"
            style={{ width: `${fillPercent(value, min, max)}%` }}
          />
        </div>
        <input
          type="range"
          className="settings-judge-weight__input"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={value}
          aria-label={label}
          onChange={(event) => onChange(Number(event.target.value))}
        />
      </div>
    </div>
  );
}

export function AgentDetailPage() {
  const { agentId = "" } = useParams();
  const { backendConnected } = useAppStore();
  const toast = useToast();
  const [agent, setAgent] = useState<AgentDetailDto | null>(null);
  const [attemptDraft, setAttemptDraft] = useState<number | null>(null);
  const [temperatureDraft, setTemperatureDraft] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [savingAttempt, setSavingAttempt] = useState(false);
  const [savingTemperature, setSavingTemperature] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedPromptId, setCopiedPromptId] = useState<string | null>(null);
  const attemptSaveToken = useRef(0);
  const temperatureSaveToken = useRef(0);

  useEffect(() => {
    if (!backendConnected || !agentId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setAttemptDraft(null);
    setTemperatureDraft(null);
    getAgent(agentId)
      .then((detail) => {
        if (cancelled) return;
        setAgent(detail);
        setAttemptDraft(detail.attempt == null ? null : clampAttempt(detail.attempt));
        setTemperatureDraft(
          detail.temperature == null ? null : roundTemperature(detail.temperature),
        );
      })
      .catch((err) => {
        if (!cancelled) setError(toAppError(err).message);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [agentId, backendConnected]);

  useEffect(() => {
    if (loading || !agent || agent.attempt == null || attemptDraft == null) return;
    if (attemptDraft === agent.attempt) return;
    const next = clampAttempt(attemptDraft);
    if (next < ATTEMPT_MIN || next > ATTEMPT_MAX) return;
    const token = attemptSaveToken.current;
    const timeout = window.setTimeout(() => {
      void (async () => {
        if (token !== attemptSaveToken.current) return;
        setSavingAttempt(true);
        try {
          const detail = await setAgentAttempt(agent.id, next);
          if (token !== attemptSaveToken.current) return;
          setAgent(detail);
          setAttemptDraft(detail.attempt);
          toast.notify("Attempt saved", "success");
        } catch (err) {
          if (token !== attemptSaveToken.current) return;
          setAttemptDraft(agent.attempt);
          toast.notify(toAppError(err).message, "error");
        } finally {
          if (token === attemptSaveToken.current) setSavingAttempt(false);
        }
      })();
    }, AUTO_SAVE_DELAY_MS);
    return () => window.clearTimeout(timeout);
  }, [agent, attemptDraft, loading, toast]);

  useEffect(() => {
    if (loading || !agent || agent.temperatureNote === "This agent does not call an LLM.") return;
    if (temperatureDraft == null || sameTemperature(temperatureDraft, agent.temperature)) return;
    const next = roundTemperature(temperatureDraft);
    const token = temperatureSaveToken.current;
    const timeout = window.setTimeout(() => {
      void (async () => {
        if (token !== temperatureSaveToken.current) return;
        setSavingTemperature(true);
        try {
          const detail = await setAgentTemperature(agent.id, next);
          if (token !== temperatureSaveToken.current) return;
          setAgent(detail);
          setTemperatureDraft(
            detail.temperature == null ? null : roundTemperature(detail.temperature),
          );
          toast.notify("Temperature saved", "success");
        } catch (err) {
          if (token !== temperatureSaveToken.current) return;
          setTemperatureDraft(
            agent.temperature == null ? null : roundTemperature(agent.temperature),
          );
          toast.notify(toAppError(err).message, "error");
        } finally {
          if (token === temperatureSaveToken.current) setSavingTemperature(false);
        }
      })();
    }, AUTO_SAVE_DELAY_MS);
    return () => window.clearTimeout(timeout);
  }, [agent, temperatureDraft, loading, toast]);

  async function copyPrompt(id: string, system: string) {
    try {
      await navigator.clipboard.writeText(system);
      setCopiedPromptId(id);
      toast.notify("Prompt copied", "success");
      window.setTimeout(() => {
        setCopiedPromptId((current) => (current === id ? null : current));
      }, 1600);
    } catch (err) {
      toast.notify(toAppError(err).message, "error");
    }
  }

  async function restorePresets() {
    if (!agent) return;
    const restoreAttempt = agent.attempt != null;
    const restoreTemperature = agent.temperatureNote !== "This agent does not call an LLM.";
    if (!restoreAttempt && !restoreTemperature) return;
    attemptSaveToken.current += 1;
    temperatureSaveToken.current += 1;
    if (restoreAttempt) setSavingAttempt(true);
    if (restoreTemperature) setSavingTemperature(true);
    try {
      let detail = agent;
      if (restoreAttempt) detail = await setAgentAttempt(agent.id, ATTEMPT_DEFAULT);
      if (restoreTemperature) detail = await setAgentTemperature(agent.id, null);
      setAgent(detail);
      setAttemptDraft(detail.attempt);
      setTemperatureDraft(
        detail.temperature == null ? null : roundTemperature(detail.temperature),
      );
      toast.notify("Defaults restored", "success");
    } catch (err) {
      toast.notify(toAppError(err).message, "error");
    } finally {
      setSavingAttempt(false);
      setSavingTemperature(false);
    }
  }

  if (!backendConnected) {
    return (
      <div className="page agents-page">
        <PageHeader title="Agent" backTo="/agents" />
        <Card>
          <EmptyState
            title="Backend offline"
            description="Connect the desktop backend to read this agent."
          />
        </Card>
      </div>
    );
  }

  return (
    <div className="page agents-page">
      <PageHeader title="Agent" backTo="/agents" backOnly />

      {error ? (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      ) : null}

      {loading && !agent ? (
        <Card>
          <p className="text-muted">Loading agent</p>
        </Card>
      ) : null}

      {agent ? (
        <>
          <section className="agents-detail__identity">
            <code className="mono agents-detail__identity-id">{agent.id}</code>
            <h1 className="agents-detail__identity-name">{agent.displayName}</h1>
            <p className="agents-detail__identity-description">{agent.description}</p>
          </section>
          <Card>
            <div className="agents-detail__metrics">
              {agent.attempt == null || attemptDraft == null ? (
                <p className="text-muted">{agent.attemptNote}</p>
              ) : (
                <MetricSlider
                  label="Attempt"
                  hint="The try limit for this agent. Range 1 to 10."
                  value={clampAttempt(attemptDraft)}
                  min={ATTEMPT_MIN}
                  max={ATTEMPT_MAX}
                  step={1}
                  display={String(clampAttempt(attemptDraft))}
                  disabled={savingAttempt || savingTemperature}
                  onChange={(value) => setAttemptDraft(clampAttempt(value))}
                />
              )}
              {agent.temperatureNote === "This agent does not call an LLM." ||
              temperatureDraft == null ? (
                <p className="text-muted">
                  {agent.temperatureNote ?? "Uses the model temperature until set."}
                </p>
              ) : (
                <MetricSlider
                  label="Temperature"
                  hint="The sampling temperature for this agent. Range 0 to 2."
                  value={temperatureDraft}
                  min={TEMP_MIN}
                  max={TEMP_MAX}
                  step={TEMP_STEP}
                  display={temperatureDraft.toFixed(1)}
                  disabled={savingAttempt || savingTemperature}
                  onChange={(value) => setTemperatureDraft(roundTemperature(value))}
                />
              )}
            </div>
            <div className="settings-section__actions">
              <Button
                variant="secondary"
                size="sm"
                disabled={savingAttempt || savingTemperature}
                onClick={() => void restorePresets()}
              >
                Restore Default
              </Button>
            </div>
          </Card>

          <Card>
            <h2 className="agents-detail__heading">Prompts</h2>
            {agent.prompts.length === 0 ? (
              <p className="text-muted">No prompts.</p>
            ) : (
              <div className="agents-detail__stack">
                {agent.prompts.map((prompt) => (
                  <section key={prompt.id}>
                    <div className="agents-detail__prompt-head">
                      <h3 className="agents-detail__id">
                        <code className="mono">{prompt.id}</code>
                      </h3>
                      <IconButton
                        ariaLabel={
                          copiedPromptId === prompt.id ? "Prompt copied" : "Copy prompt"
                        }
                        size="sm"
                        active={copiedPromptId === prompt.id}
                        onClick={() => void copyPrompt(prompt.id, prompt.system)}
                      >
                        {copiedPromptId === prompt.id ? <IconCheck /> : <IconCopy />}
                      </IconButton>
                    </div>
                    <pre className="mono agents-detail__prompt">{prompt.system}</pre>
                  </section>
                ))}
              </div>
            )}
          </Card>

          <section>
            <h2 className="agents-detail__heading">Tools</h2>
            {agent.tools.length === 0 ? (
              <Card>
                <p className="text-muted">No tools.</p>
              </Card>
            ) : (
              <div className="agents-card-grid">
                {agent.tools.map((tool) => (
                  <article key={tool.id} className="agents-card agents-card--static">
                    <code className="mono agents-card__name">{tool.id}</code>
                    <p className="agents-card__description">{tool.description}</p>
                  </article>
                ))}
              </div>
            )}
          </section>
        </>
      ) : null}
    </div>
  );
}
