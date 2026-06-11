import type { ReactNode } from "react";

import { AppStoreProvider } from "@/app/store/AppStore";
import { ErrorBoundary } from "@/shared/errors";

type AppProvidersProps = {
  children: ReactNode;
};

export function AppProviders({ children }: AppProvidersProps) {
  return (
    <ErrorBoundary>
      <AppStoreProvider>{children}</AppStoreProvider>
    </ErrorBoundary>
  );
}
