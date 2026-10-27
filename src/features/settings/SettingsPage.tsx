import { useCallback, useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { ask } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { useAppStore } from "@/app/store/AppStore";
import { Button, Card, PageHeader, ProgressBar, RefreshButton, Badge } from "@/shared/components";
import type { AppSettings } from "@/app/store/types";
import { toAppError } from "@/shared/errors";
import { clearAllAppData } from "@/shared/ipc/app";
import { clearLogs, listLogFiles, type LogFileInfoDto } from "@/shared/ipc/environment";
import { formatBytes } from "@/shared/utils/format";
import {
  APP_UPDATE_PROGRESS_EVENT,
  applyUpdateIfAvailable,
  checkForUpdate,
  getAppInfo,
  updateDownloadPercent,
  type UpdateProgressDto,
} from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import {
  getUpdateUiState,
  patchUpdateUiState,
  subscribeUpdateUiState,
} from "@/shared/runtime/updateUiState";

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

const THEME_CHOICES: { value: AppSettings["theme"]; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
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
    <Card
      className={`settings-section detail-section ${variant === "danger" ? "settings-section--danger" : ""}`}
    >
      <header className="settings-section__header">
        <h2 className="detail-section__title settings-section__title">{title}</h2>
        {description ? <p className="settings-section__lead">{description}</p> : null}
      </header>
      <div className="settings-section__body">{children}</div>
    </Card>
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

function detectClientPlatform(): string {
  if (typeof navigator === "undefined") return "Unknown";
  const platform = navigator.platform || "";
  const ua = navigator.userAgent || "";

  if (/Mac/i.test(platform) || /Mac OS X/i.test(ua)) {
    const match = ua.match(/Mac OS X (\d+[._]\d+(?:[._]\d+)?)/);
    return match ? `macOS ${match[1].replace(/_/g, ".")}` : "macOS";
  }
  if (/Win/i.test(platform) || /Windows/i.test(ua)) {
    if (/Windows NT 10\.0/.test(ua)) return "Windows 10/11";
    if (/Windows NT 6\.3/.test(ua)) return "Windows 8.1";
    if (/Windows NT 6\.2/.test(ua)) return "Windows 8";
    if (/Windows NT 6\.1/.test(ua)) return "Windows 7";
    return "Windows";
  }
  if (/Linux/i.test(platform) || /Linux/i.test(ua)) {
    return "Linux";
  }
  return platform || "Unknown";
}

function AboutPanel({
  backendConnected,
  backendVersion,
}: {
  backendConnected: boolean;
  backendVersion: string;
}) {
  const { notify } = useToast();
  const [platform, setPlatform] = useState(() => detectClientPlatform());
  const updateUi = useSyncExternalStore(subscribeUpdateUiState, getUpdateUiState, getUpdateUiState);
  const version = backendVersion || "0.1.0";

  useEffect(() => {
    let cancelled = false;
    getAppInfo()
      .then((info) => {
        if (!cancelled && info.platform) setPlatform(info.platform);
      })
      .catch(() => {
        if (!cancelled) setPlatform(detectClientPlatform());
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    void listen<UpdateProgressDto>(APP_UPDATE_PROGRESS_EVENT, (event) => {
      if (cancelled) return;
      const { phase, message } = event.payload;
      if (phase === "downloading") {
        patchUpdateUiState({
          status: message,
          downloadPct: updateDownloadPercent(event.payload),
          error: null,
        });
        return;
      }
      if (phase === "error") {
        // Toast comes from check/install handlers only — avoid duplicate toasts.
        patchUpdateUiState({
          status: null,
          downloadPct: null,
          error: null,
          busy: null,
        });
        return;
      }
      if (phase === "verifying" || phase === "installing" || phase === "relaunching") {
        patchUpdateUiState({ status: message, downloadPct: null });
      }
    }).then((fn) => {
      if (!cancelled) unlisten = fn;
      else fn();
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [notify]);

  async function handleCheckForUpdate() {
    if (!backendConnected || updateUi.busy) return;
    patchUpdateUiState({ busy: "check", error: null, status: null });
    try {
      const result = await checkForUpdate();
      patchUpdateUiState({
        latestVersion: result.latestVersion ?? null,
        updateAvailable: result.updateAvailable,
      });
      if (result.updateAvailable) {
        const msg = result.latestVersion
          ? `Version ${result.latestVersion} is available.`
          : "A newer version is available.";
        notify(msg, "success");
        // Toast only — keep Install button via updateAvailable, no status text.
        patchUpdateUiState({ status: null, error: null });
      } else {
        const reason = result.skippedReason || "PromptLab is up to date.";
        const upToDate = /already on latest|up to date/i.test(reason);
        notify(reason, upToDate ? "success" : "error");
        patchUpdateUiState({ status: null, error: null });
      }
    } catch (err) {
      const message = toAppError(err).message;
      patchUpdateUiState({ error: null, status: null });
      notify(message, "error");
    } finally {
      patchUpdateUiState({ busy: null });
    }
  }

  async function handleInstallUpdate() {
    if (!backendConnected || updateUi.busy) return;
    patchUpdateUiState({
      busy: "install",
      error: null,
      status: "Downloading and installing the new version…",
      downloadPct: 0,
    });
    try {
      const result = await applyUpdateIfAvailable();
      patchUpdateUiState({
        latestVersion: result.latestVersion ?? null,
        updateAvailable: result.updateAvailable && !result.applied,
      });
      if (result.applied) {
        patchUpdateUiState({ status: "Restarting the new version…", downloadPct: null });
        return;
      }
      const skipped = result.skippedReason || "No update was installed.";
      patchUpdateUiState({ status: skipped, downloadPct: null });
      if (!result.updateAvailable) {
        notify(skipped, "success");
        patchUpdateUiState({ status: null });
      }
    } catch (err) {
      const message = toAppError(err).message;
      patchUpdateUiState({ error: null, downloadPct: null, status: null });
      notify(message, "error");
    } finally {
      patchUpdateUiState({ busy: null });
    }
  }

  return (
    <SettingsSection
      title="Application"
      description="Build and connection details for this installation."
    >
      <dl className="about-list">
        <div>
          <dt>Application</dt>
          <dd>PromptLab</dd>
        </div>
        <div>
          <dt>Version</dt>
          <dd>v{version}</dd>
        </div>
        <div>
          <dt>Latest</dt>
          <dd>{updateUi.latestVersion ? `v${updateUi.latestVersion}` : "Not checked"}</dd>
        </div>
        <div>
          <dt>Backend</dt>
          <dd>
            {backendConnected
              ? `Rust - Connected (v${version})`
              : "Mock mode — Tauri IPC unavailable"}
          </dd>
        </div>
        <div>
          <dt>Platform</dt>
          <dd>{platform}</dd>
        </div>
      </dl>
      {updateUi.status ? <p className="text-muted text-sm">{updateUi.status}</p> : null}
      {updateUi.error ? <p className="text-danger text-sm">{updateUi.error}</p> : null}
      {updateUi.downloadPct !== null ? (
        <div className="settings-update-progress">
          <ProgressBar
            value={updateUi.downloadPct}
            size="sm"
            label="Downloading update"
          />
        </div>
      ) : null}
      <div className="settings-section__actions">
        <Button
          disabled={!backendConnected || updateUi.busy !== null}
          onClick={() => void handleCheckForUpdate()}
        >
          {updateUi.busy === "check" ? "Checking…" : "Check for updates"}
        </Button>
        {updateUi.updateAvailable ? (
          <Button
            variant="primary"
            disabled={!backendConnected || updateUi.busy !== null}
            onClick={() => void handleInstallUpdate()}
          >
            {updateUi.busy === "install" ? "Installing…" : "Install and restart"}
          </Button>
        ) : null}
      </div>
    </SettingsSection>
  );
}

function ClearLogCard({ backendConnected }: { backendConnected: boolean }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [logFiles, setLogFiles] = useState<LogFileInfoDto[] | null>(null);
  const [filesLoading, setFilesLoading] = useState(true);
  const [filesError, setFilesError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const { notify } = useToast();

  useEffect(() => {
    let cancelled = false;
    setLogFiles(null);
    setFilesError(null);
    if (!backendConnected) {
      setFilesLoading(false);
      return;
    }
    setFilesLoading(true);
    void listLogFiles()
      .then((files) => {
        if (!cancelled) setLogFiles(files);
      })
      .catch((err) => {
        if (!cancelled) setFilesError(toAppError(err).message);
      })
      .finally(() => {
        if (!cancelled) setFilesLoading(false);
      });
    return () => { cancelled = true; };
  }, [backendConnected, refreshKey]);

  async function handleClearLogs() {
    if (!backendConnected || busy) return;
    setBusy(true);
    setError(null);
    try {
      const confirmed = await ask(
        "This permanently clears application and scan console log files, including recent diagnostic events. Projects, scans, reports, models, and settings are kept.\n\nNew logs will continue to be recorded. This cannot be undone.",
        {
          title: "Clear Log",
          kind: "warning",
          okLabel: "Clear Log",
          cancelLabel: "Cancel",
        },
      );
      if (!confirmed) return;
      try {
        await clearLogs();
      } finally {
        // Reload even after a partial failure: some files may already be cleared.
        setRefreshKey((key) => key + 1);
      }
      notify("Logs cleared successfully.", "success");
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="settings-danger-card">
      <p className="text-muted text-sm">
        Clear application logs, scan console logs, and recent diagnostic events without removing
        projects, scans, reports, models, or settings. New logs continue to be recorded; no restart is needed.
      </p>
      <div className="card__header-row">
        <h3 className="card__title">Log files</h3>
        {logFiles ? (
          <span className="text-muted text-sm" aria-live="polite">
            {logFiles.length} {logFiles.length === 1 ? "file" : "files"} · Total:{" "}
            {formatBytes(logFiles.reduce((total, file) => total + file.sizeBytes, 0))}
          </span>
        ) : null}
      </div>
      {!backendConnected ? (
        <p className="text-muted text-sm">Connect to the Tauri backend to view log files.</p>
      ) : filesLoading ? (
        <p className="text-muted text-sm" role="status">Loading log files…</p>
      ) : filesError ? (
        <p className="text-danger text-sm" role="alert">{filesError}</p>
      ) : logFiles ? (
        logFiles.length === 0 ? (
          <p className="text-muted text-sm">No log files found.</p>
        ) : (
          <div className="settings-usage__table-wrap settings-log-files">
            <table className="settings-usage__table" aria-label="Log files and sizes">
              <thead>
                <tr><th scope="col">File</th><th scope="col">Size</th></tr>
              </thead>
              <tbody>
                {logFiles.map((file) => (
                  <tr key={file.path}>
                    <td title={file.path}>{file.name}</td>
                    <td className="mono">{formatBytes(file.sizeBytes)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )
      ) : null}
      {error ? <p className="text-danger text-sm" role="alert">{error}</p> : null}
      <div className="settings-section__actions">
        <Button
          variant="danger"
          disabled={!backendConnected || busy || filesLoading}
          onClick={() => void handleClearLogs()}
        >
          {busy ? "Clearing…" : "Clear Log"}
        </Button>
      </div>
    </div>
  );
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
    <div className="settings-danger-card">
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
    </div>
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
                <div className="settings-field settings-field--last">
                  <span id="theme-label" className="settings-field__label">
                    Theme
                  </span>
                  <div className="theme-choice" role="radiogroup" aria-labelledby="theme-label">
                    {THEME_CHOICES.map((choice) => {
                      const selected = settings.theme === choice.value;
                      return (
                        <button
                          key={choice.value}
                          type="button"
                          role="radio"
                          aria-checked={selected}
                          className={`btn btn--md theme-choice__btn ${selected ? "btn--primary" : "btn--secondary"}`}
                          onClick={() => update("theme", choice.value)}
                        >
                          {choice.label}
                        </button>
                      );
                    })}
                  </div>
                </div>
              </SettingsSection>

              <SettingsSection
                title="Privacy"
                description="Control optional data sent outside this device."
              >
                <label className="settings-toggle settings-toggle--disabled">
                  <input type="checkbox" disabled checked={false} readOnly />
                  <span>Anonymous usage telemetry</span>
                  <Badge variant="muted">Coming soon</Badge>
                </label>
              </SettingsSection>
            </div>
          )}

          {activeTab === "ai" && (
            <div className="settings-tab-panel settings-sections">
              <SettingsSection
                title="AI runtime"
                description="Judge, planner, and payload generation share one runtime configuration."
              >
                <p className="text-muted text-sm">
                  Configure a remote AI provider (OpenAI, Anthropic, Gemini, Azure, Bedrock, OpenRouter, or custom OpenAI-compatible) on the AI Runtime page.
                </p>
                <div className="settings-section__actions">
                  <Link className="button button--secondary" to="/runtime">
                    Open AI Runtime
                  </Link>
                </div>
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
                description="Lifetime input and output tokens consumed by AI Runtime, by agent and by model."
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
                title="Clear Log"
                description="Free up log storage while keeping your workspace data. This cannot be undone."
              >
                <ClearLogCard backendConnected={backendConnected} />
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
              <AboutPanel
                backendConnected={backendConnected}
                backendVersion={backendVersion}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
