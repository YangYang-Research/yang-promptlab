import type { ReactNode } from "react";

type IconButtonProps = {
  ariaLabel: string;
  children: ReactNode;
  active?: boolean;
  disabled?: boolean;
  variant?: "ghost" | "primary" | "danger";
  size?: "sm" | "md";
  onClick?: () => void;
  type?: "button" | "submit";
};

export function IconButton({
  ariaLabel,
  children,
  active = false,
  disabled,
  variant = "ghost",
  size = "md",
  onClick,
  type = "button",
}: IconButtonProps) {
  return (
    <button
      type={type}
      className={[
        "icon-btn",
        `icon-btn--${variant}`,
        `icon-btn--${size}`,
        active ? "icon-btn--active" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      aria-label={ariaLabel}
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
