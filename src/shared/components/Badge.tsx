import type { Severity } from "@/shared/types";

type BadgeProps = {
  children: string;
  variant?: "default" | "success" | "warning" | "danger" | "info" | "muted";
};

export function Badge({ children, variant = "default" }: BadgeProps) {
  return <span className={`badge badge--${variant}`}>{children}</span>;
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
      : status === "completed" || status === "installed"
        ? "success"
        : status === "failed" || status === "error"
          ? "danger"
          : status === "pending" || status === "available"
            ? "muted"
            : "default";

  return <Badge variant={variant}>{status.replace("_", " ")}</Badge>;
}
