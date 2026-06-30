import type { Severity } from "@/shared/types";

type BadgeProps = {
  children: string;
  variant?: "default" | "success" | "warning" | "danger" | "info" | "muted";
  className?: string;
};

export function Badge({ children, variant = "default", className }: BadgeProps) {
  const classes = ["badge", `badge--${variant}`, className].filter(Boolean).join(" ");
  return <span className={classes}>{children}</span>;
}

const severityVariant: Record<Severity, BadgeProps["variant"]> = {
  critical: "danger",
  high: "warning",
  medium: "info",
  low: "muted",
  info: "default",
};

type SeverityBadgeProps = {
  severity: Severity;
};

export function SeverityBadge({ severity }: SeverityBadgeProps) {
  return (
    <Badge variant={severityVariant[severity]}>
      {severity}
    </Badge>
  );
}

export function StatusBadge({ status }: { status: string }) {
  const variant =
    status === "running"
      ? "info"
      : status === "paused" || status === "draft"
        ? "warning"
      : status === "completed" || status === "installed" || status === "registered"
        ? "success"
        : status === "failed" || status === "error"
          ? "danger"
          : status === "cancelled"
            ? "muted"
          : status === "pending" || status === "available"
            ? "muted"
            : "default";

  return <Badge variant={variant}>{status.replace("_", " ")}</Badge>;
}
