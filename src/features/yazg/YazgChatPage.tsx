import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  PageHeader,
  PageLoadingSkeleton,
  Select,
  YazgBadge,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import { healthCheck } from "@/shared/ipc";
import {
  yazgChat,
  type YazgAgentEventDto,
  type YazgIntent,
} from "@/shared/ipc/yazg";
import { isYazgAgentLive, assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import { useToast } from "@/shared/notifications";

type ChatMessage = {
  id: string;
  role: "user" | "yazg";
  text: string;
  events?: YazgAgentEventDto[];
  intent?: string;
};

function newId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function YazgChatPage() {
  const { targets, loading: storeLoading } = useAppStore();
  const [backendConnected, setBackendConnected] = useState(false);
  const { configuration, loading: configLoading } = useAiInferenceRoute({
    enabled: backendConnected,
  });
  const { notify } = useToast();
  const live = isYazgAgentLive(configuration);

  const [targetId, setTargetId] = useState<string>("");
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: "welcome",
      role: "yazg",
      text: "I am Yazg. Select a target, then ask me to analyze the endpoint or generate an attack plan — or just say hi.",
      events: [{ agent: "yazg", kind: "info", message: "Supervisor ready" }],
    },
  ]);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    void healthCheck()
      .then(() => setBackendConnected(true))
      .catch(() => setBackendConnected(false));
  }, []);

  const targetOptions = useMemo(
    () =>
      targets.map((t) => ({
        value: t.id,
        label: `${t.name}${t.url ? ` · ${t.url}` : ""}`,
      })),
    [targets],
  );

  useEffect(() => {
    if (!targetId && targetOptions[0]) {
      setTargetId(targetOptions[0].value);
    }
  }, [targetId, targetOptions]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy]);

  const send = async (message: string, intent: YazgIntent = "auto") => {
    const trimmed = message.trim();
    if (!trimmed || busy) return;

    const yazg = await assertYazgAgentLive(backendConnected);
    if (!yazg.live) {
      notify(yazg.message, "error");
      setMessages((prev) => [
        ...prev,
        { id: newId(), role: "user", text: trimmed },
        {
          id: newId(),
          role: "yazg",
          text: yazg.message,
          events: [{ agent: "yazg", kind: "failed", message: yazg.message }],
        },
      ]);
      setDraft("");
      return;
    }

    setDraft("");
    setMessages((prev) => [
      ...prev,
      { id: newId(), role: "user", text: trimmed },
    ]);
    setBusy(true);

    try {
      const response = await yazgChat({
        message: trimmed,
        targetId: targetId || null,
        intent,
      });
      setMessages((prev) => [
        ...prev,
        {
          id: newId(),
          role: "yazg",
          text: response.reply,
          events: response.events,
          intent: response.intent,
        },
      ]);
    } catch (err) {
      const messageText = toAppError(err).message;
      notify(messageText, "error");
      setMessages((prev) => [
        ...prev,
        {
          id: newId(),
          role: "yazg",
          text: `I could not complete that request.\n\n${messageText}`,
          events: [{ agent: "yazg", kind: "failed", message: messageText }],
        },
      ]);
    } finally {
      setBusy(false);
    }
  };

  if ((storeLoading || configLoading) && messages.length <= 1) {
    return <PageLoadingSkeleton />;
  }

  return (
    <div className="yazg-chat-page">
      <PageHeader
        title="Yazg"
        description="Supervisor agent with ReAct routing to AnalyzeEndpointAgent, AttackPlanAgent, GeneratePromptAgent, RecommendAgent, SummaryAgent, and JudgeCoordinatorAgent."
        actions={
          <div className="yazg-chat-page__header-meta">
            <YazgBadge />
            <span
              className={`yazg-chat-page__live ${live ? "yazg-chat-page__live--on" : ""}`}
            >
              {live ? "Agent live" : "Offline"}
            </span>
          </div>
        }
      />

      {!backendConnected ? (
        <Card className="detail-section">
          <p className="text-muted">Connect the Tauri backend to chat with Yazg.</p>
        </Card>
      ) : null}

      <div className="yazg-chat-page__layout">
        <Card className="detail-section yazg-chat-page__sidebar">
          <h2 className="detail-section__title">Context</h2>
          <label className="yazg-chat-page__field">
            <span className="yazg-chat-page__label">Target</span>
            {targetOptions.length === 0 ? (
              <p className="text-muted text-sm">
                No targets yet.{" "}
                <Link to="/targets">Create a target</Link> or start a{" "}
                <Link to="/scans/new">scan wizard</Link>.
              </p>
            ) : (
              <Select
                value={targetId}
                onChange={(event) => setTargetId(event.target.value)}
                aria-label="Target for Yazg"
              >
                {targetOptions.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            )}
          </label>

          <div className="yazg-chat-page__actions">
            <Button
              variant="secondary"
              size="sm"
              disabled={!backendConnected || !targetId || busy || !live}
              onClick={() => void send("Analyze this endpoint", "analyze_endpoint")}
            >
              Suggest: analyze endpoint
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={!backendConnected || !targetId || busy || !live}
              onClick={() => void send("Generate an attack plan", "attack_plan")}
            >
              Suggest: attack plan
            </Button>
          </div>

          <p className="detail-section__hint">
            Buttons only hint the goal — Yazg ReActs (Reason → Act → Observe) and chooses
            AnalyzeEndpointAgent / AttackPlanAgent / GeneratePromptAgent / RecommendAgent / SummaryAgent / JudgeCoordinatorAgent.
            Wizard, Attack Factory, recommendations, project summary, and scan judging use the same hierarchy.
          </p>
        </Card>

        <Card className="detail-section yazg-chat-page__thread">
          <div className="yazg-chat-page__messages" role="log" aria-live="polite">
            {messages.map((msg) => (
              <article
                key={msg.id}
                className={`yazg-chat-bubble yazg-chat-bubble--${msg.role}`}
              >
                <header className="yazg-chat-bubble__meta">
                  {msg.role === "yazg" ? "Yazg" : "You"}
                  {msg.intent ? (
                    <span className="yazg-chat-bubble__intent">{msg.intent}</span>
                  ) : null}
                </header>
                <p className="yazg-chat-bubble__text">{msg.text}</p>
                {msg.events && msg.events.length > 0 ? (
                  <ul className="yazg-chat-events">
                    {msg.events.map((event, index) => (
                      <li key={`${msg.id}-${index}`} className="yazg-chat-events__item">
                        <span className="yazg-chat-events__agent">
                          {event.agent}
                        </span>
                        <span className="yazg-chat-events__kind">{event.kind}</span>
                        <span className="yazg-chat-events__msg">{event.message}</span>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </article>
            ))}
            {busy ? (
              <p className="yazg-chat-page__typing text-muted text-sm">Yazg is working…</p>
            ) : null}
            <div ref={bottomRef} />
          </div>

          <form
            className="yazg-chat-page__composer"
            onSubmit={(event) => {
              event.preventDefault();
              void send(draft, "auto");
            }}
          >
            <textarea
              className="yazg-chat-page__input"
              rows={2}
              placeholder="Message Yazg…"
              value={draft}
              disabled={!backendConnected || busy}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void send(draft, "auto");
                }
              }}
            />
            <Button
              type="submit"
              variant="primary"
              disabled={!backendConnected || busy || !draft.trim()}
            >
              Send
            </Button>
          </form>
        </Card>
      </div>
    </div>
  );
}
