import type { ReactNode } from "react";

import { AppStoreProvider } from "@/app/store/AppStore";
import { ErrorBoundary } from "@/shared/errors";
import { ToastProvider } from "@/shared/notifications";

type AppProvidersProps = {
  children: ReactNode;
};

export function AppProviders({ children }: AppProvidersProps) {
  return (
    <ErrorBoundary>
      <ToastProvider>
        <AppStoreProvider>{children}</AppStoreProvider>
      </ToastProvider>
    </ErrorBoundary>
  );
}
