import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { AppRouter } from "@/app/router/AppRouter";
import { useAppStore } from "@/app/store/AppStore";
import { BrandMark } from "@/shared/components/BrandMark";
import { ProgressBar } from "@/shared/components";
import {
  applyUpdateIfAvailable,
  APP_UPDATE_PROGRESS_EVENT,
  getAppInfo,
  getStartupStatus,
  healthCheck,
  updateDownloadPercent,
  type UpdateProgressDto,
} from "@/shared/ipc";
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
  const [relaunching, setRelaunching] = useState(false);
  const [bootMessage, setBootMessage] = useState("Starting security workspace…");
  const [downloadPct, setDownloadPct] = useState<number | null>(null);
  const [bootFailure, setBootFailure] = useState<BootFailure | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        log.info("bootstrapping frontend");

        try {
          let unlisten: UnlistenFn | undefined;
          try {
            unlisten = await listen<UpdateProgressDto>(APP_UPDATE_PROGRESS_EVENT, (event) => {
              if (cancelled) return;
              setBootMessage(event.payload.message);
              setDownloadPct(updateDownloadPercent(event.payload));
            });
          } catch {
            unlisten = undefined;
          }

          const update = await applyUpdateIfAvailable();
          await unlisten?.();
          if (cancelled) return;
          if (update.applied) {
            setRelaunching(true);
            setBootMessage("Restarting the new version…");
            setDownloadPct(null);
            log.info("update applied; waiting for relaunch", { update });
            return;
          }
          setDownloadPct(null);
          setBootMessage("Starting security workspace…");
        } catch (updateError) {
          log.warn("startup update check skipped", {
            error: toAppError(updateError),
          });
          if (!cancelled) {
            setDownloadPct(null);
            setBootMessage("Starting security workspace…");
          }
        }

        try {
          const startup = await getStartupStatus();
          if (!startup.ok) {
            if (cancelled) return;
            setBootFailure({
              message: startup.databaseError ?? "Error: database could not be opened.",
              databasePath: startup.databasePath ?? undefined,
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

  if (!ready || relaunching) {
    return (
      <div className="boot-screen">
        <div className="boot-screen__logo" aria-hidden="true">
          <BrandMark size={56} />
        </div>
        <h1 className="boot-screen__title">PromptLab</h1>
        <div className="page-loader__spinner" />
        <p className="boot-screen__subtitle">{bootMessage}</p>
        {downloadPct !== null ? (
          <div className="boot-screen__progress">
            <ProgressBar value={downloadPct} size="sm" label="Downloading update" />
          </div>
        ) : null}
      </div>
    );
  }

  if (bootFailure) {
    return (
      <div className="boot-screen boot-screen--error" role="alert">
        <div className="boot-screen__logo" aria-hidden="true">
          <BrandMark size={56} />
        </div>
        <h1 className="boot-screen__title">Application error</h1>
        <p className="boot-screen__subtitle">
          PromptLab could not open or migrate its database.
        </p>
        <pre className="boot-screen__error">{bootFailure.message}</pre>
      </div>
    );
  }

  return <AppRouter />;
}

export function App() {
  return <AppBootstrap />;
}
