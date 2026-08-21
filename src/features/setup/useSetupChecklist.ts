import { useCallback, useEffect, useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import { getRuntimeConfiguration } from "@/shared/ipc/runtime";

import {
  deriveSetupSteps,
  setupProgress,
  type SetupProgressInput,
  type SetupStep,
} from "./setupSteps";
import { isSetupComplete, markSetupComplete } from "./setupStorage";

const POLL_MS = 4000;

function emptyProgress(): SetupProgressInput {
  return {
    mode: "not_configured",
    initialized: false,
    localModelCount: 0,
    configuredThirdPartyCount: 0,
    selectedModelId: null,
    modelLoaded: false,
    projectCount: 0,
    scanCount: 0,
  };
}

export function useSetupChecklist(): {
  visible: boolean;
  steps: SetupStep[];
  doneCount: number;
  total: number;
  refresh: () => void;
} {
  const { projects, scans, models, backendConnected, loading } = useAppStore();
  const [dismissed, setDismissed] = useState(() => isSetupComplete());
  const [runtime, setRuntime] = useState<SetupProgressInput>(() => emptyProgress());

  const refreshRuntime = useCallback(async () => {
    if (!backendConnected) {
      setRuntime((prev) => ({
        ...prev,
        mode: "not_configured",
        initialized: false,
        modelLoaded: false,
      }));
      return;
    }
    try {
      const configuration = await getRuntimeConfiguration();
      const settings = configuration.settings;
      setRuntime({
        mode: configuration.mode,
        initialized: settings.initialized,
        localModelCount: Math.max(settings.localModels.length, models.length),
        configuredThirdPartyCount: settings.thirdPartyModels.filter((m) => m.configured)
          .length,
        selectedModelId: settings.selectedModelId,
        modelLoaded: configuration.runtimeStatus.modelLoaded,
        projectCount: projects.length,
        scanCount: scans.length,
      });
    } catch {
      setRuntime((prev) => ({
        ...prev,
        projectCount: projects.length,
        scanCount: scans.length,
        localModelCount: Math.max(prev.localModelCount, models.length),
      }));
    }
  }, [backendConnected, models.length, projects.length, scans.length]);

  useEffect(() => {
    if (dismissed || loading) return;
    void refreshRuntime();
  }, [dismissed, loading, refreshRuntime]);

  useEffect(() => {
    if (dismissed || !backendConnected) return;
    const id = window.setInterval(() => {
      void refreshRuntime();
    }, POLL_MS);
    const onFocus = () => {
      void refreshRuntime();
    };
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, [backendConnected, dismissed, refreshRuntime]);

  const steps = useMemo(
    () =>
      deriveSetupSteps({
        ...runtime,
        projectCount: projects.length,
        scanCount: scans.length,
        localModelCount: Math.max(runtime.localModelCount, models.length),
      }),
    [runtime, projects.length, scans.length, models.length],
  );

  const { doneCount, total, allDone } = setupProgress(steps);

  useEffect(() => {
    if (dismissed || loading || !allDone) return;
    markSetupComplete();
    setDismissed(true);
  }, [allDone, dismissed, loading]);

  return {
    visible: !dismissed && !loading && backendConnected,
    steps,
    doneCount,
    total,
    refresh: () => {
      void refreshRuntime();
    },
  };
}
