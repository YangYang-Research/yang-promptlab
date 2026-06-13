import { IconRefresh } from "./Icons";
import { IconButton } from "./IconButton";

type RefreshButtonProps = {
  onClick: () => void;
  disabled?: boolean;
  loading?: boolean;
  ariaLabel?: string;
  size?: "sm" | "md";
};

export function RefreshButton({
  onClick,
  disabled,
  loading = false,
  ariaLabel = "Refresh",
  size = "md",
}: RefreshButtonProps) {
  return (
    <IconButton
      ariaLabel={loading ? "Refreshing…" : ariaLabel}
      disabled={disabled || loading}
      size={size}
      onClick={onClick}
    >
      <IconRefresh className={loading ? "icon-refresh--spinning" : undefined} />
    </IconButton>
  );
}
