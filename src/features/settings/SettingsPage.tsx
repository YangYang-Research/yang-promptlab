import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { ask } from "@tauri-apps/plugin-dialog";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Card, PageHeader, Select } from "@/shared/components";
import type { AppSettings } from "@/app/store/types";
import { toAppError } from "@/shared/errors";
import { clearAllAppData } from "@/shared/ipc/app";
import { getModelsRegistryDiagnostics, type ModelRegistryDiagnosticsDto } from "@/shared/ipc/models";
import {
  securityAudit,
  securityMigrateSecrets,
  type SecretMigrationAudit,
} from "@/shared/ipc/security";

import { RegistryDiagnosticsPanel } from "./RegistryDiagnosticsPanel";

type SettingsTab = "general" | "troubleshooting" | "security" | "paths" | "about";

const SETTINGS_TABS: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "troubleshooting", label: "Troubleshooting" },
  { id: "security", label: "Security" },
  { id: "paths", label: "Paths" },
  { id: "about", label: "About" },
];

function clearBrowserAppStorage() {
  if (typeof window === "undefined") return;
  const keysToRemove: string[] = [];
  for (let i = 0; i < window.localStorage.length; i += 1) {
    const key = window.localStorage.key(i);
    if (key?.startsWith("aisec:")) keysToRemove.push(key);
  }
  for (const key of keysToRemove) {
    window.localStorage.removeItem(key);
  }
  window.sessionStorage.removeItem("aisec:scan-wizard");
}

function ClearAllDataCard({ backendConnected }: { backendConnected: boolean }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleClearAllData() {
    if (!backendConnected || busy) return;

    const confirmed = await ask(
      "This permanently deletes all AISec data on this device — projects, targets, scans, findings, reports, models, runtime state, and cached wizard sessions — then restarts the app.\n\nThis cannot be undone.",
      {
        title: "Clear All Data",
        kind: "warning",
        okLabel: "Clear All Data",
        cancelLabel: "Cancel",
      },
    );
    if (!confirmed) return;

    setBusy(true);
    setError(null);
    try {
      clearBrowserAppStorage();
      await clearAllAppData();
    } catch (err) {
      setError(toAppError(err).message);
      setBusy(false);
    }
  }

  return (
    <Card className="settings-danger-card">
      <h3 className="card__title">Data</h3>
      <p className="text-muted text-sm">
        Remove all application data from this device and restart AISec with a fresh workspace.
      </p>
      {error ? <p className="text-danger text-sm">{error}</p> : null}
      <Button
        variant="danger"
        disabled={!backendConnected || busy}
        onClick={() => void handleClearAllData()}
      >
        {busy ? "Clearing…" : "Clear All Data"}
      </Button>
    </Card>
  );
}

function SecuritySecretsCard({ backendConnected }: { backendConnected: boolean }) {
  const [audit, setAudit] = useState<SecretMigrationAudit | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastMigrated, setLastMigrated] = useState<number | null>(null);

  useEffect(() => {
    if (!backendConnected) {
      setAudit(null);
      return;
    }
    void securityAudit()
      .then(setAudit)
      .catch(() => setAudit(null));
  }, [backendConnected, lastMigrated]);

  async function handleMigrate() {
    setBusy(true);
    setError(null);
    try {
      const report = await securityMigrateSecrets();
      setAudit(report.auditAfter);
      setLastMigrated(
        report.authMigrated +
          report.targetsMigrated +
          report.storageMigrated +
          report.judgeMigrated,
      );
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  const legacyCount = audit?.legacyCount ?? 0;

  return (
    <>
      <p className="text-muted text-sm">
        Move plaintext credentials from targets, auth profiles, sessions, and legacy judge config into the
        OS keychain and encrypted session vault.
      </p>
      {!backendConnected ? (
        <p className="text-muted text-sm">Connect to the Tauri backend to audit secrets.</p>
      ) : audit ? (
        <dl className="about-list">
          <div>
            <dt>Legacy records</dt>
            <dd>{legacyCount === 0 ? "None" : legacyCount}</dd>
          </div>
          {legacyCount > 0 ? (
            <>
              <div>
                <dt>Targets</dt>
                <dd>{audit.targetsLegacy}</dd>
              </div>
              <div>
                <dt>Auth profiles</dt>
                <dd>{audit.authProfilesLegacy}</dd>
              </div>
              <div>
                <dt>Sessions</dt>
                <dd>{audit.sessionsLegacy + audit.sessionStorageLegacy}</dd>
              </div>
              <div>
                <dt>Judge config</dt>
                <dd>{audit.judgeConfigLegacy}</dd>
              </div>
            </>
          ) : null}
        </dl>
      ) : null}
      {error ? <p className="text-danger text-sm">{error}</p> : null}
      <Button
        variant="secondary"
        disabled={!backendConnected || busy || legacyCount === 0}
        onClick={() => void handleMigrate()}
      >
        {busy ? "Migrating…" : "Migrate Secrets"}
      </Button>
    </>
  );
}

function TroubleshootingTab({ backendConnected }: { backendConnected: boolean }) {
  const [diagnostics, setDiagnostics] = useState<ModelRegistryDiagnosticsDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!backendConnected) {
      setDiagnostics(null);
      return;
    }
    setLoading(true);
    setError(null);
    void getModelsRegistryDiagnostics()
      .then(setDiagnostics)
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected]);

  return (
    <div className="settings-tab-panel">
      <Card>
        <h3 className="card__title">Registry Diagnostics</h3>
        {!backendConnected ? (
          <p className="text-muted text-sm">Connect to the Tauri backend to run registry diagnostics.</p>
        ) : loading ? (
          <p className="text-muted text-sm">Loading diagnostics…</p>
        ) : error ? (
          <p className="text-danger text-sm">{error}</p>
        ) : diagnostics ? (
          <RegistryDiagnosticsPanel diagnostics={diagnostics} />
        ) : null}
      </Card>
    </div>
  );
}

export function SettingsPage() {
  const { settings, dispatch, backendVersion, backendConnected } = useAppStore();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    dispatch({ type: "UPDATE_SETTING", key, value });
  };

  return (
    <div className="page settings-page">
      <PageHeader
        title="Settings"
        description="Application preferences and workspace configuration"
      />

      <div className="settings-tabs" role="tablist" aria-label="Settings sections">
        {SETTINGS_TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.id}
            className={`settings-tabs__btn ${activeTab === tab.id ? "settings-tabs__btn--active" : ""}`}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === "general" && (
        <div className="settings-tab-panel">
          <div className="settings-grid">
            <Card>
              <h3 className="card__title">Appearance</h3>
              <div className="settings-field">
                <label htmlFor="theme">Theme</label>
                <Select
                  id="theme"
                  value={settings.theme}
                  onChange={(e) => update("theme", e.target.value as AppSettings["theme"])}
                >
                  <option value="dark">Dark</option>
                  <option value="light">Light</option>
                  <option value="system">System</option>
                </Select>
              </div>
            </Card>

            <Card>
              <h3 className="card__title">AI Runtime</h3>
              <p className="text-muted text-sm">
                All AI features (judge, planner, payload generator) use the single AI Runtime
                configuration. Choose local llama.cpp or a third-party API model on the AI Runtime
                page.
              </p>
              <Link className="button button--secondary" to="/runtime">
                Open AI Runtime
              </Link>
            </Card>

            <Card>
              <h3 className="card__title">Security Testing</h3>
              <label className="settings-toggle">
                <input
                  type="checkbox"
                  checked={settings.offlineMode}
                  onChange={(e) => update("offlineMode", e.target.checked)}
                />
                <span>Offline mode (local models only)</span>
              </label>
              <label className="settings-toggle">
                <input
                  type="checkbox"
                  checked={settings.autoJudge}
                  onChange={(e) => update("autoJudge", e.target.checked)}
                />
                <span>Auto-run AI judge after attacks (uses AI Runtime)</span>
              </label>
              <label className="settings-toggle">
                <input
                  type="checkbox"
                  checked={settings.telemetry}
                  onChange={(e) => update("telemetry", e.target.checked)}
                />
                <span>Anonymous usage telemetry</span>
              </label>
            </Card>

            <ClearAllDataCard backendConnected={backendConnected} />
          </div>
        </div>
      )}

      {activeTab === "troubleshooting" && (
        <TroubleshootingTab backendConnected={backendConnected} />
      )}

      {activeTab === "security" && (
        <div className="settings-tab-panel">
          <div className="settings-grid">
            <Card>
              <h3 className="card__title">Secret Migration</h3>
              <SecuritySecretsCard backendConnected={backendConnected} />
            </Card>
          </div>
        </div>
      )}

      {activeTab === "paths" && (
        <div className="settings-tab-panel">
          <div className="settings-grid">
            <Card>
              <h3 className="card__title">Workspace Paths</h3>
              <div className="settings-field">
                <label htmlFor="pluginsDir">Plugins directory</label>
                <input
                  id="pluginsDir"
                  className="input mono"
                  value={settings.pluginsDir}
                  onChange={(e) => update("pluginsDir", e.target.value)}
                />
              </div>
              <div className="settings-field">
                <label htmlFor="modelsDir">Models directory</label>
                <input
                  id="modelsDir"
                  className="input mono"
                  value={settings.modelsDir}
                  onChange={(e) => update("modelsDir", e.target.value)}
                />
              </div>
            </Card>
          </div>
        </div>
      )}

      {activeTab === "about" && (
        <div className="settings-tab-panel">
          <div className="settings-grid">
            <Card>
              <h3 className="card__title">About AISec</h3>
              <dl className="about-list">
                <div>
                  <dt>Application</dt>
                  <dd>AISec Desktop v0.1.0</dd>
                </div>
                <div>
                  <dt>Backend</dt>
                  <dd>
                    {backendConnected
                      ? `Connected (v${backendVersion})`
                      : "Mock mode — Tauri IPC unavailable"}
                  </dd>
                </div>
                <div>
                  <dt>Platform</dt>
                  <dd>Offline-first AI Security Testing</dd>
                </div>
              </dl>
              <Button variant="ghost">View Logs</Button>
            </Card>
          </div>
        </div>
      )}
    </div>
  );
}
