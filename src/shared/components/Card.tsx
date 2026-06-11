import type { ReactNode } from "react";

type CardProps = {
  children: ReactNode;
  className?: string;
  padding?: "sm" | "md" | "none";
};

export function Card({ children, className = "", padding = "md" }: CardProps) {
  return (
    <div className={`card card--pad-${padding} ${className}`.trim()}>
      {children}
    </div>
  );
}

type StatCardProps = {
  label: string;
  value: string | number;
  hint?: string;
  trend?: "up" | "down" | "neutral";
  accent?: "default" | "critical" | "success" | "warning";
};

export function StatCard({ label, value, hint, trend, accent = "default" }: StatCardProps) {
  return (
    <Card className={`stat-card stat-card--${accent}`}>
      <span className="stat-card__label">{label}</span>
      <span className="stat-card__value">{value}</span>
      {hint && (
        <span className={`stat-card__hint stat-card__hint--${trend ?? "neutral"}`}>
          {hint}
        </span>
      )}
    </Card>
  );
}
