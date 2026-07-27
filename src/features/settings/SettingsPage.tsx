import { useCallback, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { ask } from "@tauri-apps/plugin-dialog";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Card, PageHeader, RefreshButton, Select, Badge } from "@/shared/components";
import type { AppSettings } from "@/app/store/types";
import { toAppError } from "@/shared/errors";
import { clearAllAppData } from "@/shared/ipc/app";

import { EnvironmentsPanel } from "./EnvironmentsPanel";
import { JudgeRoleWeightsPanel } from "./JudgeRoleWeightsPanel";
import { ProxySettingsPanel } from "./ProxySettingsPanel";
import { RuntimeInferencePanel } from "./RuntimeInferencePanel";
import { TroubleshootingPanel } from "./TroubleshootingPanel";
import { UsagePanel } from "./UsagePanel";

type SettingsTab =
  | "general"
  | "ai"
  | "usage"
  | "network"
  | "storage"
  | "diagnostics"
  | "about";

const SETTINGS_SECTIONS: {
  id: SettingsTab;
  label: string;
  hint: string;
}[] = [
  { id: "general", label: "General", hint: "Theme and privacy" },
  { id: "ai", label: "AI Runtime", hint: "Model and inference" },
  { id: "usage", label: "Usage", hint: "Token consumption" },
  { id: "network", label: "Network", hint: "Proxy and connectivity" },
  { id: "storage", label: "Data & storage", hint: "Paths and reset" },
  { id: "diagnostics", label: "Diagnostics", hint: "Logs and health" },
  { id: "about", label: "About", hint: "Version info" },
];

type SettingsSectionProps = {
  title: string;
  description?: string;
  variant?: "default" | "danger";
  children: ReactNode;
};

function SettingsSection({
  title,
  description,
  variant = "default",
  children,
}: SettingsSectionProps) {
  return (
    <section
      className={`settings-section ${variant === "danger" ? "settings-section--danger" : ""}`}
    >
      <header className="settings-section__header">
        <h2 className="settings-section__title">{title}</h2>
        {description ? <p className="settings-section__lead">{description}</p> : null}
      </header>
      <div className="settings-section__body">{children}</div>
    </section>
  );
}

function clearBrowserAppStorage() {
  if (typeof window === "undefined") return;
  const keysToRemove: string[] = [];
  for (let i = 0; i < window.localStorage.length; i += 1) {
    const key = window.localStorage.key(i);
    if (key?.startsWith("promptlab:")) keysToRemove.push(key);
  }
  for (const key of keysToRemove) {
    window.localStorage.removeItem(key);
  }
  window.sessionStorage.removeItem("promptlab:scan-wizard");
}

function ClearAllDataCard({ backendConnected }: { backendConnected: boolean }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleClearAllData() {
    if (!backendConnected || busy) return;

    const confirmed = await ask(
      "This permanently deletes all PromptLab data on this device — projects, targets, scans, findings, reports, models, runtime state, and cached wizard sessions — then restarts the app.\n\nThis cannot be undone.",
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
      <p className="text-muted text-sm">
        Remove all application data from this device and restart PromptLab with a fresh workspace.
      </p>
      {error ? <p className="text-danger text-sm">{error}</p> : null}
      <div className="settings-section__actions">
        <Button
          variant="danger"
          disabled={!backendConnected || busy}
          onClick={() => void handleClearAllData()}
        >
          {busy ? "Clearing…" : "Clear All Data"}
        </Button>
      </div>
    </Card>
  );
}

export function SettingsPage() {
  const { settings, dispatch, backendVersion, backendConnected, loading, error, actions } =
    useAppStore();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [usageRefreshKey, setUsageRefreshKey] = useState(0);
  const [usageLoading, setUsageLoading] = useState(false);

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    dispatch({ type: "UPDATE_SETTING", key, value });
  };

  const handleUsageLoadingChange = useCallback((next: boolean) => {
    setUsageLoading(next);
  }, []);

  async function handleRefresh() {
    setUsageRefreshKey((key) => key + 1);
    await actions.refresh();
  }

  return (
    <div className="page settings-page">
      <PageHeader
        title="Settings"
        description="Preferences, workspace paths, and diagnostics for this device"
        actions={
          <RefreshButton
            loading={loading || usageLoading}
            error={error}
            onClick={() => void handleRefresh()}
          />
        }
      />

      <div className="settings-layout">
        <nav className="settings-nav" aria-label="Settings sections">
          {SETTINGS_SECTIONS.map((section) => (
            <button
              key={section.id}
              type="button"
              className={`settings-nav__btn ${activeTab === section.id ? "settings-nav__btn--active" : ""}`}
              aria-current={activeTab === section.id ? "page" : undefined}
              onClick={() => setActiveTab(section.id)}
            >
              <span className="settings-nav__label">{section.label}</span>
              <span className="settings-nav__hint">{section.hint}</span>
            </button>
          ))}
        </nav>

        <div className="settings-content">
          {activeTab === "general" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="Appearance"
                description="Choose how PromptLab looks on this device."
              >
                <Card>
                  <div className="settings-field settings-field--last">
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
              </SettingsSection>

              <SettingsSection
                title="Privacy"
                description="Control optional data sent outside this device."
              >
                <Card>
                  <label className="settings-toggle settings-toggle--disabled">
                    <input type="checkbox" disabled checked={false} readOnly />
                    <span>Anonymous usage telemetry</span>
                    <Badge variant="muted">Coming soon</Badge>
                  </label>
                </Card>
              </SettingsSection>
            </div>
          )}

          {activeTab === "ai" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="AI runtime"
                description="Judge, planner, and payload generation share one runtime configuration."
              >
                <Card>
                  <p className="text-muted text-sm">
                    Configure local llama.cpp or a third-party API model on the AI Runtime page.
                  </p>
                  <div className="settings-section__actions">
                    <Link className="button button--secondary" to="/runtime">
                      Open AI Runtime
                    </Link>
                  </div>
                </Card>
              </SettingsSection>

              <SettingsSection
                title="Judge worker weights"
                description="Relative influence of JudgeWorker, ClassifierWorker, and AttackerWorker when aggregating scan confidence."
              >
                <JudgeRoleWeightsPanel disabled={!backendConnected} />
              </SettingsSection>

              <SettingsSection
                title="Runtime inference"
                description="Where the app invokes your AI Runtime model — verification, planning, execution, results, and Attack Factory."
              >
                <RuntimeInferencePanel />
              </SettingsSection>
            </div>
          )}

          {activeTab === "usage" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="Token usage"
                description="Lifetime input and output tokens consumed by AI Runtime, broken down by Yazg agent and sub-agent."
              >
                <UsagePanel
                  backendConnected={backendConnected}
                  refreshKey={usageRefreshKey}
                  onLoadingChange={handleUsageLoadingChange}
                />
              </SettingsSection>
            </div>
          )}

          {activeTab === "network" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="Proxy"
                description="Route outbound HTTP(S) and SOCKS traffic through a corporate or local proxy."
              >
                <ProxySettingsPanel backendConnected={backendConnected} />
              </SettingsSection>
            </div>
          )}

          {activeTab === "storage" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="Workspace location"
                description="All projects, models, logs, and runtime state live under this root directory."
              >
                <EnvironmentsPanel backendConnected={backendConnected} />
              </SettingsSection>

              <SettingsSection
                title="Danger zone"
                description="Permanently remove local data. This cannot be undone."
                variant="danger"
              >
                <ClearAllDataCard backendConnected={backendConnected} />
              </SettingsSection>
            </div>
          )}

          {activeTab === "diagnostics" && (
            <TroubleshootingPanel backendConnected={backendConnected} />
          )}

          {activeTab === "about" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="Application"
                description="Build and connection details for this installation."
              >
                <Card>
                  <dl className="about-list">
                    <div>
                      <dt>Application</dt>
                      <dd>PromptLab Desktop v0.1.0</dd>
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
                      <dd>Offline-first AI Security Testing Platform</dd>
                    </div>
                  </dl>
                </Card>
              </SettingsSection>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
