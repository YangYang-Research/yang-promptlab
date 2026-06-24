import { useCallback, useEffect, useRef, useState } from "react";

import { useToast } from "@/shared/notifications";

import { IconRefresh } from "./Icons";
import { IconButton } from "./IconButton";

const MIN_SPIN_MS = 3000;
const DEFAULT_SUCCESS_MESSAGE = "Latest data loaded";

type RefreshButtonProps = {
  onClick: () => void;
  disabled?: boolean;
  loading?: boolean;
  error?: string | null;
  ariaLabel?: string;
  size?: "sm" | "md";
  successMessage?: string;
  showSuccessToast?: boolean;
};

export function RefreshButton({
  onClick,
  disabled,
  loading = false,
  error = null,
  ariaLabel = "Refresh",
  size = "md",
  successMessage = DEFAULT_SUCCESS_MESSAGE,
  showSuccessToast = true,
}: RefreshButtonProps) {
  const { notify } = useToast();
  const [clickedAt, setClickedAt] = useState<number | null>(null);
  const pendingSuccessToastRef = useRef(false);

  const spinning =
    clickedAt !== null && (loading || Date.now() - clickedAt < MIN_SPIN_MS);

  const finishRefresh = useCallback(() => {
    if (pendingSuccessToastRef.current) {
      pendingSuccessToastRef.current = false;
      if (showSuccessToast && !error) {
        notify(successMessage, "success");
      }
    }
    setClickedAt(null);
  }, [error, notify, showSuccessToast, successMessage]);

  useEffect(() => {
    if (clickedAt === null) return;

    const elapsed = Date.now() - clickedAt;

    if (!loading && elapsed >= MIN_SPIN_MS) {
      finishRefresh();
      return;
    }

    if (!loading && elapsed < MIN_SPIN_MS) {
      const timer = setTimeout(finishRefresh, MIN_SPIN_MS - elapsed);
      return () => clearTimeout(timer);
    }
  }, [clickedAt, loading, finishRefresh]);

  function handleClick() {
    setClickedAt(Date.now());
    pendingSuccessToastRef.current = true;
    onClick();
  }

  return (
    <IconButton
      ariaLabel={spinning ? "Refreshing…" : ariaLabel}
      disabled={disabled || spinning}
      size={size}
      onClick={handleClick}
    >
      <IconRefresh className={spinning ? "icon-refresh--spinning" : undefined} />
    </IconButton>
  );
}
