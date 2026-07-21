import { useEffect } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  applyResolvedTheme,
  applyThemePreference,
  saveThemePreference,
  subscribeSystemTheme,
} from "@/shared/theme/theme";

export function ThemeSync() {
  const { settings } = useAppStore();
  const theme = settings.theme;

  useEffect(() => {
    applyThemePreference(theme);
    saveThemePreference(theme);
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;
    return subscribeSystemTheme((resolved) => applyResolvedTheme(resolved));
  }, [theme]);

  return null;
}
