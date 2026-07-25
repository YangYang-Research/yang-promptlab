import { categoryLabel } from "@/features/scans/categoryLabel";
import type { Finding } from "@/shared/types";

export type CategoryBar = {
  id: string;
  label: string;
  count: number;
  color: string;
};

/** Stable colors keyed by attack category — severity-adjacent, teal-friendly. */
const CATEGORY_COLOR_BY_ID: Record<string, string> = {
  prompt_injection: "#c72929",
  system_prompt_extraction: "#f47f1f",
  jailbreak: "#eab308",
  rag_leakage: "#1975d2",
  memory_poisoning: "#0d9488",
  cross_user_leakage: "#4cae4f",
  agent_goal_hijacking: "#b45309",
  tool_abuse: "#64748b",
  mcp_abuse: "#0891b2",
};

const FALLBACK_COLORS = [
  "#c72929",
  "#f47f1f",
  "#eab308",
  "#4cae4f",
  "#1975d2",
  "#0d9488",
  "#b45309",
  "#64748b",
  "#0891b2",
  "#78716c",
];

function colorForCategory(id: string, index: number): string {
  return CATEGORY_COLOR_BY_ID[id] ?? FALLBACK_COLORS[index % FALLBACK_COLORS.length];
}

export function buildFindingsByCategory(findings: Finding[]): CategoryBar[] {
  const counts = new Map<string, number>();
  for (const finding of findings) {
    const id = finding.category?.trim() || "unknown";
    counts.set(id, (counts.get(id) ?? 0) + 1);
  }

  return [...counts.entries()]
    .map(([id, count], index) => ({
      id,
      label: categoryLabel(id),
      count,
      color: colorForCategory(id, index),
    }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}

type FindingsByCategoryChartProps = {
  data: CategoryBar[];
  size?: number;
};

function polarPoint(cx: number, cy: number, r: number, angleRad: number) {
  return {
    x: cx + r * Math.cos(angleRad),
    y: cy + r * Math.sin(angleRad),
  };
}

/** Equal-angle wedge; radius scales with value (polar area). */
function polarAreaPath(
  cx: number,
  cy: number,
  r: number,
  startAngle: number,
  endAngle: number,
): string {
  const start = polarPoint(cx, cy, r, startAngle);
  const end = polarPoint(cx, cy, r, endAngle);
  const largeArc = endAngle - startAngle > Math.PI ? 1 : 0;
  return [
    `M ${cx} ${cy}`,
    `L ${start.x} ${start.y}`,
    `A ${r} ${r} 0 ${largeArc} 1 ${end.x} ${end.y}`,
    "Z",
  ].join(" ");
}

export function FindingsByCategoryChart({ data, size = 200 }: FindingsByCategoryChartProps) {
  const bars = data.filter((item) => item.count > 0);
  const total = bars.reduce((sum, item) => sum + item.count, 0);
  const max = bars.reduce((peak, item) => Math.max(peak, item.count), 0);

  if (total === 0 || max === 0) {
    return <p className="text-muted text-sm">No findings by attack category yet.</p>;
  }

  const cx = size / 2;
  const cy = size / 2;
  const maxR = size * 0.38;
  const slice = (Math.PI * 2) / bars.length;
  const startOffset = -Math.PI / 2;
  const guideLevels = [0.35, 0.65, 1];

  return (
    <div className="category-polar" role="img" aria-label="Findings by attack category polar area chart">
      <div className="category-polar__chart" style={{ width: size, height: size }}>
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
          <defs>
            {bars.map((item) => (
              <radialGradient key={`pg-${item.id}`} id={`polar-grad-${item.id}`} cx="50%" cy="50%" r="70%">
                <stop offset="0%" stopColor={item.color} stopOpacity="0.35" />
                <stop offset="55%" stopColor={item.color} stopOpacity="0.78" />
                <stop offset="100%" stopColor={item.color} stopOpacity="0.95" />
              </radialGradient>
            ))}
          </defs>

          {guideLevels.map((level) => (
            <circle
              key={level}
              className="category-polar__guide"
              cx={cx}
              cy={cy}
              r={maxR * level}
              fill="none"
            />
          ))}

          {bars.map((_, index) => {
            const angle = startOffset + index * slice;
            const tip = polarPoint(cx, cy, maxR, angle);
            return (
              <line
                key={`spoke-${index}`}
                className="category-polar__spoke"
                x1={cx}
                y1={cy}
                x2={tip.x}
                y2={tip.y}
              />
            );
          })}

          {bars.map((item, index) => {
            const r = Math.max((item.count / max) * maxR, maxR * 0.12);
            const a0 = startOffset + index * slice;
            const a1 = a0 + slice;
            return (
              <path
                key={item.id}
                className="category-polar__slice"
                d={polarAreaPath(cx, cy, r, a0, a1)}
                fill={`url(#polar-grad-${item.id})`}
                stroke={item.color}
                style={{ animationDelay: `${index * 55}ms` }}
              >
                <title>{`${item.label}: ${item.count}`}</title>
              </path>
            );
          })}

          <circle className="category-polar__hub" cx={cx} cy={cy} r={size * 0.11} />
          <text x={cx} y={cy - 2} textAnchor="middle" className="category-polar__total">
            {total}
          </text>
          <text x={cx} y={cy + 11} textAnchor="middle" className="category-polar__total-label">
            total
          </text>
        </svg>
      </div>

      <ul className="category-polar__legend">
        {bars.map((item) => {
          const share = Math.round((item.count / total) * 100);
          return (
            <li key={item.id} className="category-polar__legend-item">
              <span className="category-polar__swatch" style={{ background: item.color }} aria-hidden />
              <span className="category-polar__legend-label" title={item.label}>
                {item.label}
              </span>
              <span className="category-polar__legend-meta">
                <strong>{item.count}</strong>
                <span>{share}%</span>
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
