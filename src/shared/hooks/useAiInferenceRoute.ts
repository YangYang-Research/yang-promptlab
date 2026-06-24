import { useCallback, useEffect, useRef, useState } from "react";

import { toAppError } from "@/shared/errors";
import {
  getRuntimeConfiguration,
  setRuntimeInferenceRoute,
  type AiInferenceRoute,
  type AiInferenceSettingsDto,
  type RuntimeConfigurationDto,
} from "@/shared/ipc/runtime";

type UseAiInferenceRouteOptions = {
  enabled?: boolean;
};

export function useAiInferenceRoute(options: UseAiInferenceRouteOptions = {}) {
  const { enabled = true } = options;
  const [configuration, setConfiguration] = useState<RuntimeConfigurationDto | null>(null);
  const [initialLoading, setInitialLoading] = useState(enabled);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const hasLoadedOnceRef = useRef(false);

  const settings: AiInferenceSettingsDto | null = configuration?.settings ?? null;

  const refresh = useCallback(async () => {
    const dto = await getRuntimeConfiguration();
    setConfiguration(dto);
    setError(null);
    hasLoadedOnceRef.current = true;
    return dto;
  }, []);

  useEffect(() => {
    if (!enabled) {
      setInitialLoading(false);
      return;
    }
    if (!hasLoadedOnceRef.current) {
      setInitialLoading(true);
    }
    void refresh()
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setInitialLoading(false));
  }, [enabled, refresh]);

  const setRoute = useCallback(
    async (route: AiInferenceRoute, selectedModelId?: string | null) => {
      setBusy(true);
      setError(null);
      try {
        const settingsDto = await setRuntimeInferenceRoute({
          route,
          selectedModelId: selectedModelId ?? undefined,
        });
        const dto = await refresh();
        return { configuration: dto, settings: settingsDto };
      } catch (err) {
        const message = toAppError(err).message;
        setError(message);
        throw err;
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  return {
    configuration,
    settings,
    mode: configuration?.mode ?? "not_configured",
    route: settings?.initialized ? settings.route : null,
    loading: initialLoading,
    busy,
    error,
    refresh,
    setRoute,
  };
}
