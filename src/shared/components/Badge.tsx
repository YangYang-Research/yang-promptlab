import type { ReactNode } from "react";

import { categoryLabel } from "@/features/scans/categoryLabel";
import type { Severity } from "@/shared/types";
import type { TargetScanStatusLabel } from "@/shared/targetScanContext";

import { IconCheck, IconX } from "./Icons";

type BadgeProps = {
  children: ReactNode;
  variant?: "default" | "success" | "warning" | "danger" | "info" | "muted";
  className?: string;
};

export function Badge({ children, variant = "default", className }: BadgeProps) {
  const classes = ["badge", `badge--${variant}`, className].filter(Boolean).join(" ");
  return <span className={classes}>{children}</span>;
}

type SeverityBadgeProps = {
  severity: Severity;
};

export function SeverityBadge({ severity }: SeverityBadgeProps) {
  return <span className={`badge badge--severity-${severity}`}>{severity}</span>;
}

const SEVERITY_LEVELS: Severity[] = ["critical", "high", "medium", "low", "info"];

type PriorityBadgeProps = {
  priority: string;
};

/** Same palette as SeverityBadge (critical / high / medium / low / info). */
export function PriorityBadge({ priority }: PriorityBadgeProps) {
  const normalized = priority.trim().toLowerCase() as Severity;
  if (SEVERITY_LEVELS.includes(normalized)) {
    return <SeverityBadge severity={normalized} />;
  }
  return <Badge variant="muted">{priority}</Badge>;
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
          normalized === "ok" ||
          normalized === "success" ||
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

  const label = status.replace(/_/g, " ");
  if (variant === "success") {
    return (
      <Badge variant={variant} className="badge--with-icon">
        <IconCheck className="badge__icon" />
        {label}
      </Badge>
    );
  }
  if (variant === "danger") {
    return (
      <Badge variant={variant} className="badge--with-icon">
        <IconX className="badge__icon" />
        {label}
      </Badge>
    );
  }

  return <Badge variant={variant}>{label}</Badge>;
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

const AUTH_KIND_CLASS: Record<"none" | "basic" | "api_key" | "jwt", string> = {
  none: "badge--auth-none",
  basic: "badge--auth-basic",
  api_key: "badge--auth-api-key",
  jwt: "badge--auth-jwt",
};

type AuthTypeBadgeProps = {
  kind: "none" | "basic" | "api_key" | "jwt";
  label: string;
};

/** Badge colors aligned with Scan wizard Authentication Type buttons. */
export function AuthTypeBadge({ kind, label }: AuthTypeBadgeProps) {
  return <span className={["badge", "badge--auth", AUTH_KIND_CLASS[kind]].join(" ")}>{label}</span>;
}

const ATTACK_CATEGORY_CLASS: Record<string, string> = {
  prompt_injection: "badge--category-prompt-injection",
  system_prompt_extraction: "badge--category-system-prompt-extraction",
  jailbreak: "badge--category-jailbreak",
  rag_leakage: "badge--category-rag-leakage",
  memory_poisoning: "badge--category-memory-poisoning",
  cross_user_leakage: "badge--category-cross-user-leakage",
  agent_goal_hijacking: "badge--category-agent-goal-hijacking",
  tool_abuse: "badge--category-tool-abuse",
  mcp_abuse: "badge--category-mcp-abuse",
};

function attackCategoryClass(category: string): string {
  const id = category.trim().toLowerCase().replace(/[\s-]+/g, "_");
  return ATTACK_CATEGORY_CLASS[id] ?? "badge--category-unknown";
}

type AttackCategoryBadgeProps = {
  category: string;
};

/** Per-category colors so Attack Category badges don't sink into the zinc/teal theme. */
export function AttackCategoryBadge({ category }: AttackCategoryBadgeProps) {
  return (
    <span className={["badge", "badge--category", attackCategoryClass(category)].join(" ")}>
      {categoryLabel(category)}
    </span>
  );
}
