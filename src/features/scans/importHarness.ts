/** Seconds to show Loading countdown before auto-advancing import steps. */
export const IMPORT_STEP_COUNTDOWN_SEC = 3;

/** Max automatic Verification attempts during import harness. */
export const IMPORT_VERIFY_MAX_ATTEMPTS = 5;

/** Delay between failed verify retries. */
export const IMPORT_VERIFY_RETRY_DELAY_MS = 1_500;

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}
