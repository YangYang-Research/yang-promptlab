import { useEffect, useState } from "react";

import { AppRouter } from "@/app/router/AppRouter";
import { useAppStore } from "@/app/store/AppStore";
import { BrandMark } from "@/shared/components/BrandMark";
import { getAppInfo, getStartupStatus, healthCheck } from "@/shared/ipc";
import { createLogger } from "@/shared/logging";
import { toAppError } from "@/shared/errors";

const log = createLogger("App");

type BootFailure = {
  message: string;
  databasePath?: string;
};

function AppBootstrap() {
  const { dispatch } = useAppStore();
  const [ready, setReady] = useState(false);
  const [bootFailure, setBootFailure] = useState<BootFailure | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        log.info("bootstrapping frontend");

        try {
          const startup = await getStartupStatus();
          if (!startup.ok) {
            if (cancelled) return;
            setBootFailure({
              message:
                startup.databaseError ??
                "The application database could not be opened.",
              databasePath: startup.databasePath,
            });
            dispatch({
              type: "SET_BACKEND",
              version: "0.1.0",
              connected: false,
            });
            return;
          }
        } catch (startupError) {
          // Older backends without startup_status — fall through to health.
          log.warn("startup_status unavailable", {
            error: toAppError(startupError),
          });
        }

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
          if (appError.code === "STORAGE" || /database|migration|schema/i.test(appError.message)) {
            setBootFailure({ message: appError.message });
          }
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

  if (bootFailure) {
    return (
      <div className="boot-screen boot-screen--error" role="alert">
        <div className="boot-screen__logo" aria-hidden="true">
          <BrandMark size={56} />
        </div>
        <h1 className="boot-screen__title">Database error</h1>
        <p className="boot-screen__subtitle">
          PromptLab could not open or migrate its database. The app stayed open so you can
          recover without a silent crash.
        </p>
        <pre className="boot-screen__error">{bootFailure.message}</pre>
        {bootFailure.databasePath ? (
          <p className="boot-screen__path">
            Database path: <code>{bootFailure.databasePath}</code>
          </p>
        ) : null}
        <p className="boot-screen__hint">
          Back up or delete the database file, then restart PromptLab.
        </p>
      </div>
    );
  }

  return <AppRouter />;
}

export function App() {
  return <AppBootstrap />;
}
