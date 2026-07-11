import type { Severity } from "@/shared/types";
import type { TargetScanStatusLabel } from "@/shared/targetScanContext";

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

const targetScanStatusVariant: Record<TargetScanStatusLabel, BadgeProps["variant"]> = {
  "Never Scanned": "muted",
  Running: "info",
  Completed: "success",
  Failed: "danger",
};

export function TargetScanStatusBadge({ label }: { label: TargetScanStatusLabel }) {
  return <Badge variant={targetScanStatusVariant[label]}>{label}</Badge>;
}

export function StatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase();
  const variant =
    normalized === "running"
      ? "info"
      : normalized === "paused" || normalized === "draft" || normalized === "pending"
        ? "warning"
      : normalized === "completed" ||
          normalized === "installed" ||
          normalized === "registered" ||
          normalized === "verified" ||
          normalized === "scanned"
        ? "success"
        : normalized === "failed" || normalized === "error"
          ? "danger"
          : normalized === "cancelled"
            ? "muted"
          : normalized === "available"
            ? "muted"
            : "default";

  return <Badge variant={variant}>{status.replace(/_/g, " ")}</Badge>;
}
