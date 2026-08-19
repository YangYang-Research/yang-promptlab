import type { Severity } from "@/shared/types";

export type DoughnutSlice = {
  id: string;
  label: string;
  count: number;
  color: string;
};

export type SeveritySlice = {
  severity: Severity;
  label: string;
  count: number;
  color: string;
};

/** Severity Spectrum — https://colormagic.app/palette/67bd867d5ce83338b13894fc */
const SEVERITY_COLORS: Record<Severity, string> = {
  critical: "#c72929",
  high: "#f47f1f",
  medium: "#ffb300",
  low: "#4cae4f",
  info: "#1975d2",
};

type DoughnutChartProps = {
  data: DoughnutSlice[];
  size?: number;
  emptyMessage?: string;
  ariaLabel?: string;
};

export function DoughnutChart({
  data,
  size = 168,
  emptyMessage = "No data yet.",
  ariaLabel = "Doughnut chart",
}: DoughnutChartProps) {
  const slices = data.filter((item) => item.count > 0);
  const total = slices.reduce((sum, item) => sum + item.count, 0);

  if (total === 0) {
    return <p className="text-muted text-sm">{emptyMessage}</p>;
  }

  const cx = size / 2;
  const cy = size / 2;
  const radius = size * 0.36;
  const strokeWidth = size * 0.14;
  const circumference = 2 * Math.PI * radius;

  let offset = 0;

  return (
    <div className="category-doughnut">
      <div className="category-doughnut__chart" style={{ width: size, height: size }}>
        <svg
          width={size}
          height={size}
          viewBox={`0 0 ${size} ${size}`}
          role="img"
          aria-label={ariaLabel}
        >
          <circle
            cx={cx}
            cy={cy}
            r={radius}
            fill="none"
            stroke="var(--bg-elevated)"
            strokeWidth={strokeWidth}
          />
          {slices.map((item) => {
            const fraction = item.count / total;
            const length = fraction * circumference;
            const dashOffset = -offset;
            offset += length;
            return (
              <circle
                key={item.id}
                cx={cx}
                cy={cy}
                r={radius}
                fill="none"
                stroke={item.color}
                strokeWidth={strokeWidth}
                strokeDasharray={`${length} ${circumference - length}`}
                strokeDashoffset={dashOffset}
                transform={`rotate(-90 ${cx} ${cy})`}
              />
            );
          })}
          <text x={cx} y={cy - 4} textAnchor="middle" className="category-doughnut__total">
            {total}
          </text>
          <text x={cx} y={cy + 14} textAnchor="middle" className="category-doughnut__total-label">
            findings
          </text>
        </svg>
      </div>
      <ul className="category-doughnut__legend">
        {slices.map((item) => (
          <li key={item.id} className="category-doughnut__legend-item">
            <span
              className="category-doughnut__swatch"
              style={{ background: item.color }}
              aria-hidden
            />
            <span className="category-doughnut__legend-label">{item.label}</span>
            <span className="category-doughnut__legend-count">{item.count}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

type SeverityDoughnutChartProps = {
  data: SeveritySlice[];
  size?: number;
};

export function SeverityDoughnutChart({ data, size = 168 }: SeverityDoughnutChartProps) {
  return (
    <DoughnutChart
      data={data.map((item) => ({
        id: item.severity,
        label: item.label,
        count: item.count,
        color: item.color,
      }))}
      size={size}
      emptyMessage="No findings by severity yet."
      ariaLabel="Findings by severity doughnut chart"
    />
  );
}

export function severitySliceColor(severity: Severity): string {
  return SEVERITY_COLORS[severity];
}
