import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  PageHeader,
  Select,
} from "@/shared/components";
import type { AppSettings } from "@/app/store/types";
import { getJudgeConfig, saveJudgeConfig, type JudgeConfigDto } from "@/shared/ipc/judge";
import { listModels, type ModelEntryDto } from "@/shared/ipc/models";
import { toAppError } from "@/shared/errors";

export function SettingsPage() {
  const { settings, dispatch, backendVersion, backendConnected } = useAppStore();
  const [judgeConfig, setJudgeConfig] = useState<JudgeConfigDto | null>(null);
  const [installedModels, setInstalledModels] = useState<ModelEntryDto[]>([]);
  const [judgeBusy, setJudgeBusy] = useState(false);
  const [judgeError, setJudgeError] = useState<string | null>(null);

  useEffect(() => {
    if (!backendConnected) return;
    void Promise.all([getJudgeConfig(), listModels()])
      .then(([config, models]) => {
        setJudgeConfig(config);
        setInstalledModels(models);
      })
      .catch(() => {
        setJudgeConfig(null);
        setInstalledModels([]);
      });
  }, [backendConnected]);

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    dispatch({ type: "UPDATE_SETTING", key, value });
  };

  async function handleJudgeModelChange(modelId: string) {
    if (!judgeConfig) return;
    setJudgeBusy(true);
    setJudgeError(null);
    try {
      const next =
        modelId === "none"
          ? { ...judgeConfig, mode: "deterministic" as const, localVaultModelId: null }
          : {
              ...judgeConfig,
              mode: "local_llm" as const,
              localVaultModelId: modelId,
            };
      const saved = await saveJudgeConfig(next);
      setJudgeConfig(saved);
    } catch (err) {
      setJudgeError(toAppError(err).message);
    } finally {
      setJudgeBusy(false);
    }
  }

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
          <h3 className="card__title">AI Models</h3>
          <div className="settings-field">
            <label htmlFor="judgeModel">Judge Model</label>
            <Select
              id="judgeModel"
              value={judgeConfig?.localVaultModelId ?? "none"}
              disabled={!backendConnected || judgeBusy || !judgeConfig}
              onChange={(e) => void handleJudgeModelChange(e.target.value)}
            >
              <option value="none">None (Deterministic rules only)</option>
              {installedModels.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name}
                </option>
              ))}
            </Select>
          </div>
          {judgeError ? <p className="text-danger text-sm">{judgeError}</p> : null}
          {!backendConnected ? (
            <p className="text-muted text-sm">Connect to the Tauri backend to configure judge models.</p>
          ) : null}
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
