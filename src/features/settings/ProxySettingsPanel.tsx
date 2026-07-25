import { useCallback, useEffect, useRef, useState } from "react";

import { Button, Card, Select } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  DEFAULT_PROXY_SETTINGS,
  getProxySettings,
  setProxySettings,
  testProxyConnection,
  type ProxySettingsDto,
} from "@/shared/ipc/proxy";
import { useToast } from "@/shared/notifications";

const AUTO_SAVE_DELAY_MS = 500;

const PROXY_SCHEMES = ["http", "https", "socks4", "socks4a", "socks5", "socks5h"] as const;
type ProxyScheme = (typeof PROXY_SCHEMES)[number];

type ProxyServerParts = {
  scheme: ProxyScheme;
  host: string;
  port: string;
};

type ProxySettingsPanelProps = {
  backendConnected: boolean;
};

function isProxyScheme(value: string): value is ProxyScheme {
  return (PROXY_SCHEMES as readonly string[]).includes(value);
}

function defaultPortForScheme(_scheme: ProxyScheme): string {
  return "8080";
}

function parseProxyUrl(url: string): ProxyServerParts {
  const trimmed = url.trim();
  if (!trimmed) {
    return { scheme: "http", host: "127.0.0.1", port: "8080" };
  }
  try {
    const parsed = new URL(trimmed.includes("://") ? trimmed : `http://${trimmed}`);
    const scheme = isProxyScheme(parsed.protocol.replace(":", ""))
      ? (parsed.protocol.replace(":", "") as ProxyScheme)
      : "http";
    return {
      scheme,
      host: parsed.hostname || "127.0.0.1",
      port: parsed.port || defaultPortForScheme(scheme),
    };
  } catch {
    return { scheme: "http", host: trimmed || "127.0.0.1", port: "8080" };
  }
}

function composeProxyUrl(parts: ProxyServerParts): string {
  const host = parts.host.trim();
  const port = parts.port.trim();
  if (!host || !port) return "";
  return `${parts.scheme}://${host}:${port}`;
}

function normalizeDraft(settings: ProxySettingsDto): ProxySettingsDto {
  return {
    ...DEFAULT_PROXY_SETTINGS,
    ...settings,
    username: settings.username ?? "",
    password: settings.password ?? "",
  };
}

function toPersistPayload(draft: ProxySettingsDto): ProxySettingsDto {
  return {
    ...draft,
    username: draft.username?.trim() ? draft.username : null,
    password: draft.password ? draft.password : null,
  };
}

function sameSettings(a: ProxySettingsDto, b: ProxySettingsDto): boolean {
  return (
    a.enabled === b.enabled &&
    a.url === b.url &&
    (a.username ?? "") === (b.username ?? "") &&
    (a.password ?? "") === (b.password ?? "") &&
    a.noProxy === b.noProxy &&
    a.testUrl === b.testUrl &&
    a.allowInsecureTls === b.allowInsecureTls
  );
}

function canPersist(draft: ProxySettingsDto, server: ProxyServerParts): boolean {
  if (!draft.enabled) return true;
  const port = Number(server.port.trim());
  return (
    server.host.trim().length > 0 &&
    Number.isInteger(port) &&
    port >= 1 &&
    port <= 65535 &&
    composeProxyUrl(server).length > 0
  );
}

export function ProxySettingsPanel({ backendConnected }: ProxySettingsPanelProps) {
  const { notify } = useToast();
  const [draft, setDraft] = useState<ProxySettingsDto>(DEFAULT_PROXY_SETTINGS);
  const [saved, setSaved] = useState<ProxySettingsDto>(DEFAULT_PROXY_SETTINGS);
  const [server, setServer] = useState<ProxyServerParts>(() => parseProxyUrl(""));
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testMessage, setTestMessage] = useState<string | null>(null);
  const [testOk, setTestOk] = useState<boolean | null>(null);
  const skipEnableToastRef = useRef(false);
  const hydrateRef = useRef(true);

  const load = useCallback(async () => {
    if (!backendConnected) {
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const settings = normalizeDraft(await getProxySettings());
      setDraft(settings);
      setSaved(settings);
      setServer(parseProxyUrl(settings.url));
      hydrateRef.current = true;
    } catch (err) {
      notify(toAppError(err).message, "error");
    } finally {
      setLoading(false);
    }
  }, [backendConnected, notify]);

  useEffect(() => {
    void load();
  }, [load]);

  const dirty = !sameSettings(draft, saved);

  useEffect(() => {
    if (!backendConnected || loading || !dirty || !canPersist(draft, server)) return;

    const timeout = window.setTimeout(() => {
      void (async () => {
        setSaving(true);
        try {
          const next = normalizeDraft(await setProxySettings(toPersistPayload(draft)));
          setDraft(next);
          setSaved(next);
          setServer(parseProxyUrl(next.url));
          if (hydrateRef.current) {
            hydrateRef.current = false;
          } else if (next.enabled !== saved.enabled || skipEnableToastRef.current) {
            skipEnableToastRef.current = false;
            notify(
              next.enabled
                ? "Proxy enabled — outbound traffic will use it"
                : "Proxy disabled — connections go direct",
              "success",
            );
          }
        } catch (err) {
          notify(toAppError(err).message, "error");
        } finally {
          setSaving(false);
        }
      })();
    }, AUTO_SAVE_DELAY_MS);

    return () => window.clearTimeout(timeout);
  }, [backendConnected, dirty, draft, loading, notify, saved.enabled, server]);

  function updateField<K extends keyof ProxySettingsDto>(key: K, value: ProxySettingsDto[K]) {
    setDraft((prev) => ({ ...prev, [key]: value }));
    setTestMessage(null);
    setTestOk(null);
  }

  function updateServer<K extends keyof ProxyServerParts>(key: K, value: ProxyServerParts[K]) {
    setServer((prev) => {
      const next = { ...prev, [key]: value };
      const url = composeProxyUrl(next);
      setDraft((draftPrev) => ({ ...draftPrev, url }));
      return next;
    });
    setTestMessage(null);
    setTestOk(null);
  }

  function handleToggle(enabled: boolean) {
    if (enabled) skipEnableToastRef.current = true;
    setDraft((prev) => ({
      ...prev,
      enabled,
      url: enabled ? composeProxyUrl(server) : prev.url,
    }));
    setTestMessage(null);
    setTestOk(null);
  }

  async function handleTest() {
    if (!backendConnected || testing || !draft.enabled) return;
    setTesting(true);
    setTestMessage(null);
    setTestOk(null);
    try {
      const payload = toPersistPayload({
        ...draft,
        url: composeProxyUrl(server),
      });
      const result = await testProxyConnection(payload);
      setTestOk(result.ok);
      setTestMessage(
        `${result.message} (${result.latencyMs} ms${
          result.status != null ? `, HTTP ${result.status}` : ""
        })`,
      );
      notify(
        result.ok
          ? result.viaProxy
            ? "Proxy connection OK"
            : "Direct connection OK"
          : "Proxy test failed",
        result.ok ? "success" : "error",
      );
    } catch (err) {
      const message = toAppError(err).message;
      setTestOk(false);
      setTestMessage(message);
      notify(message, "error");
    } finally {
      setTesting(false);
    }
  }

  const disabled = !backendConnected || loading || saving;
  const serverReady = canPersist({ ...draft, enabled: true }, server);

  return (
    <Card>
      <p className="text-muted text-sm">
        When enabled, outbound app traffic (AI Runtime APIs, attack HTTP, discovery, model
        downloads) is routed through this proxy. Changes save automatically.
      </p>

      <div className="settings-switch-row">
        <div className="settings-switch-row__copy">
          <span className="settings-switch-row__label">Use proxy for outbound traffic</span>
          <span className="settings-switch-row__hint">
            {draft.enabled ? "Proxy routing on" : "Direct connections"}
            {saving ? " · Saving…" : ""}
          </span>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={draft.enabled}
          aria-label="Use proxy for outbound traffic"
          className={`settings-switch${draft.enabled ? " settings-switch--on" : ""}`}
          disabled={!backendConnected || loading}
          onClick={() => handleToggle(!draft.enabled)}
        >
          <span className="settings-switch__thumb" />
        </button>
      </div>

      {draft.enabled ? (
        <div className="settings-proxy-form">
          <div className="settings-proxy-server">
            <span className="settings-proxy-server__label" id="proxy-server-label">
              Proxy Server
            </span>
            <div
              className="settings-proxy-server__row"
              role="group"
              aria-labelledby="proxy-server-label"
            >
              <Select
                id="proxy-scheme"
                aria-label="Scheme"
                value={server.scheme}
                disabled={disabled}
                onChange={(e) => {
                  const scheme = isProxyScheme(e.target.value) ? e.target.value : "http";
                  updateServer("scheme", scheme);
                }}
              >
                {PROXY_SCHEMES.map((scheme) => (
                  <option key={scheme} value={scheme}>
                    {scheme}
                  </option>
                ))}
              </Select>
              <span className="settings-proxy-server__sep" aria-hidden>
                ://
              </span>
              <input
                id="proxy-host"
                className="input mono"
                aria-label="IP or hostname"
                placeholder="127.0.0.1"
                value={server.host}
                disabled={disabled}
                onChange={(e) => updateServer("host", e.target.value)}
              />
              <span className="settings-proxy-server__sep" aria-hidden>
                :
              </span>
              <input
                id="proxy-port"
                className="input mono"
                aria-label="Port"
                placeholder="8080"
                inputMode="numeric"
                value={server.port}
                disabled={disabled}
                onChange={(e) => updateServer("port", e.target.value.replace(/[^\d]/g, ""))}
              />
            </div>
          </div>

          <div className="settings-switch-row settings-switch-row--nested">
            <div className="settings-switch-row__copy">
              <span className="settings-switch-row__label">Allow insecure TLS</span>
              <span className="settings-switch-row__hint">
                Needed for MITM proxies (Charles, mitmproxy, Burp) with self-signed CAs
              </span>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={draft.allowInsecureTls}
              aria-label="Allow insecure TLS"
              className={`settings-switch${draft.allowInsecureTls ? " settings-switch--on" : ""}`}
              disabled={disabled}
              onClick={() => updateField("allowInsecureTls", !draft.allowInsecureTls)}
            >
              <span className="settings-switch__thumb" />
            </button>
          </div>

          <div className="settings-field">
            <label htmlFor="proxy-username">Username (optional)</label>
            <input
              id="proxy-username"
              className="input"
              autoComplete="off"
              value={draft.username ?? ""}
              disabled={disabled}
              onChange={(e) => updateField("username", e.target.value)}
            />
          </div>

          <div className="settings-field">
            <label htmlFor="proxy-password">Password (optional)</label>
            <input
              id="proxy-password"
              className="input"
              type="password"
              autoComplete="new-password"
              value={draft.password ?? ""}
              disabled={disabled}
              onChange={(e) => updateField("password", e.target.value)}
            />
          </div>

          <div className="settings-field">
            <label htmlFor="proxy-no-proxy">No Proxy</label>
            <input
              id="proxy-no-proxy"
              className="input mono"
              placeholder="localhost,127.0.0.1,.internal"
              value={draft.noProxy}
              disabled={disabled}
              onChange={(e) => updateField("noProxy", e.target.value)}
            />
            <span className="text-muted text-sm">Comma-separated hosts that bypass the proxy.</span>
          </div>

          <div className="settings-field settings-field--last">
            <label htmlFor="proxy-test-url">Test URL</label>
            <input
              id="proxy-test-url"
              className="input mono"
              placeholder="https://www.google.com/generate_204"
              value={draft.testUrl}
              disabled={disabled}
              onChange={(e) => updateField("testUrl", e.target.value)}
            />
          </div>

          {testMessage ? (
            <p className={`text-sm ${testOk ? "text-muted" : "text-danger"}`}>{testMessage}</p>
          ) : null}

          <div className="settings-section__actions">
            <Button
              variant="secondary"
              disabled={disabled || testing || !serverReady}
              onClick={() => void handleTest()}
            >
              {testing ? "Testing…" : "Test Connection"}
            </Button>
          </div>
        </div>
      ) : null}
    </Card>
  );
}
