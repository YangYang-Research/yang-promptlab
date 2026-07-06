import type { Severity } from "@/shared/types";

export type SeveritySlice = {
  severity: Severity;
  label: string;
  count: number;
  color: string;
};

const SEVERITY_COLORS: Record<Severity, string> = {
  critical: "#ef4444",
  high: "#f97316",
  medium: "#eab308",
  low: "#6366f1",
  info: "#94a3b8",
};

type SeverityDoughnutChartProps = {
  data: SeveritySlice[];
  size?: number;
};

export function SeverityDoughnutChart({ data, size = 168 }: SeverityDoughnutChartProps) {
  const slices = data.filter((item) => item.count > 0);
  const total = slices.reduce((sum, item) => sum + item.count, 0);

  if (total === 0) {
    return <p className="text-muted text-sm">No findings by severity yet.</p>;
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
          aria-label="Findings by severity doughnut chart"
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
                key={item.severity}
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
          <li key={item.severity} className="category-doughnut__legend-item">
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

export function severitySliceColor(severity: Severity): string {
  return SEVERITY_COLORS[severity];
}
