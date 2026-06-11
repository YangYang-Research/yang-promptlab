import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  PageHeader,
} from "@/shared/components";
import type { AppSettings } from "@/app/store/types";

export function SettingsPage() {
  const { settings, dispatch, backendVersion, backendConnected, projects, ui } = useAppStore();

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    dispatch({ type: "UPDATE_SETTING", key, value });
  };

  return (
    <div className="page settings-page">
      <PageHeader
        title="Settings"
        description="Application preferences and workspace configuration"
      />

      <div className="settings-grid">
        <Card>
          <h3 className="card__title">General</h3>
          <div className="settings-field">
            <label htmlFor="theme">Theme</label>
            <select
              id="theme"
              className="settings-select"
              value={settings.theme}
              onChange={(e) => update("theme", e.target.value as AppSettings["theme"])}
            >
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              <option value="system">System</option>
            </select>
          </div>
          <div className="settings-field">
            <label htmlFor="project">Default Project</label>
            <select
              id="project"
              className="settings-select"
              value={ui.selectedProjectId ?? ""}
              onChange={(e) =>
                dispatch({ type: "SET_SELECTED_PROJECT", projectId: e.target.value || null })
              }
            >
              {projects.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
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
            <span>Auto-run judge after attacks</span>
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

        <Card>
          <h3 className="card__title">Paths</h3>
          <div className="settings-field">
            <label htmlFor="pluginsDir">Plugins directory</label>
            <input
              id="pluginsDir"
              className="settings-input mono"
              value={settings.pluginsDir}
              onChange={(e) => update("pluginsDir", e.target.value)}
            />
          </div>
          <div className="settings-field">
            <label htmlFor="modelsDir">Models directory</label>
            <input
              id="modelsDir"
              className="settings-input mono"
              value={settings.modelsDir}
              onChange={(e) => update("modelsDir", e.target.value)}
            />
          </div>
        </Card>

        <Card>
          <h3 className="card__title">About</h3>
          <dl className="about-list">
            <div>
              <dt>Application</dt>
              <dd>AISec Desktop v0.1.0</dd>
            </div>
            <div>
              <dt>Backend</dt>
              <dd>
                {backendConnected ? `Connected (v${backendVersion})` : "Mock mode — Tauri IPC unavailable"}
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
  );
}
