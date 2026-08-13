import { useEffect, useState } from "react";

import { AppRouter } from "@/app/router/AppRouter";
import { useAppStore } from "@/app/store/AppStore";
import { BrandMark } from "@/shared/components/BrandMark";
import { getAppInfo, healthCheck } from "@/shared/ipc";
import { createLogger } from "@/shared/logging";
import { toAppError } from "@/shared/errors";

const log = createLogger("App");

function AppBootstrap() {
  const { dispatch } = useAppStore();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        log.info("bootstrapping frontend");
        const health = await healthCheck();
        const info = await getAppInfo();

        if (cancelled) return;

        log.info("backend health", { health, info });
        dispatch({
          type: "SET_BACKEND",
          version: info.version,
          connected: true,
        });
      } catch (error) {
        const appError = toAppError(error);
        log.warn("backend unavailable", { error: appError });
        if (!cancelled) {
          dispatch({
            type: "SET_BACKEND",
            version: "0.1.0",
            connected: false,
          });
        }
      } finally {
        if (!cancelled) {
          setReady(true);
        }
      }
    }

    void bootstrap();

    return () => {
      cancelled = true;
    };
  }, [dispatch]);

  if (!ready) {
    return (
      <div className="boot-screen">
        <div className="boot-screen__logo" aria-hidden="true">
          <BrandMark size={56} />
        </div>
        <h1 className="boot-screen__title">PromptLab</h1>
        <div className="page-loader__spinner" />
        <p className="boot-screen__subtitle">Starting security workspace…</p>
      </div>
    );
  }

  return <AppRouter />;
}

export function App() {
  return <AppBootstrap />;
}
