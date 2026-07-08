import type { ReactNode } from "react";

import { AppStoreProvider } from "@/app/store/AppStore";
import { RuntimeModelLoadingPoller } from "@/app/providers/RuntimeModelLoadingPoller";
import { ThemeSync } from "@/app/providers/ThemeSync";
import { ErrorBoundary } from "@/shared/errors";
import { ToastProvider } from "@/shared/notifications";

type AppProvidersProps = {
  children: ReactNode;
};

export function AppProviders({ children }: AppProvidersProps) {
  return (
    <ErrorBoundary>
      <ToastProvider>
        <AppStoreProvider>
          <ThemeSync />
          <RuntimeModelLoadingPoller />
          {children}
        </AppStoreProvider>
      </ToastProvider>
    </ErrorBoundary>
  );
}
