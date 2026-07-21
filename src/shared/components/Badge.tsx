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

const findingStatusVariant: Record<
  "open" | "confirmed" | "false_positive" | "fixed",
  BadgeProps["variant"]
> = {
  open: "info",
  confirmed: "danger",
  false_positive: "muted",
  fixed: "success",
};

type FindingStatus = keyof typeof findingStatusVariant;

type FindingStatusBadgeProps = {
  status: FindingStatus | string;
};

export function FindingStatusBadge({ status }: FindingStatusBadgeProps) {
  const key = status.toLowerCase().replace(/[\s-]/g, "_") as FindingStatus;
  const variant = findingStatusVariant[key] ?? "default";
  return <Badge variant={variant}>{status.replace(/_/g, " ")}</Badge>;
}

const AUTH_KIND_CLASS: Record<
  "none" | "username_password" | "sso" | "basic" | "api_key" | "jwt",
  string
> = {
  none: "badge--auth-none",
  username_password: "badge--auth-username",
  sso: "badge--auth-sso",
  basic: "badge--auth-basic",
  api_key: "badge--auth-api-key",
  jwt: "badge--auth-jwt",
};

type AuthTypeBadgeProps = {
  kind: "none" | "username_password" | "sso" | "basic" | "api_key" | "jwt";
  label: string;
};

/** Badge colors aligned with Scan wizard Authentication Type buttons. */
export function AuthTypeBadge({ kind, label }: AuthTypeBadgeProps) {
  return <span className={["badge", "badge--auth", AUTH_KIND_CLASS[kind]].join(" ")}>{label}</span>;
}
