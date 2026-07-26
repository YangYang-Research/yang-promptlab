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
};

const CHART_W = 360;
const CHART_H = 210;
const PAD = { top: 24, right: 12, bottom: 54, left: 28 };

function shortLabel(label: string): string {
  const first = label.split(/\s+/)[0] ?? label;
  if (first.length <= 10) return first;
  return `${first.slice(0, 9)}…`;
}

export function FindingsByCategoryChart({ data }: FindingsByCategoryChartProps) {
  const bars = data.filter((item) => item.count > 0);
  const total = bars.reduce((sum, item) => sum + item.count, 0);
  const max = bars.reduce((peak, item) => Math.max(peak, item.count), 0);

  if (total === 0 || max === 0) {
    return <p className="text-muted text-sm">No findings by attack category yet.</p>;
  }

  const plotW = CHART_W - PAD.left - PAD.right;
  const plotH = CHART_H - PAD.top - PAD.bottom;
  const gap = bars.length > 6 ? 6 : 10;
  const barW = Math.min(40, (plotW - gap * (bars.length - 1)) / bars.length);
  const yMax = Math.max(1, Math.ceil(max * 1.15));
  const ticks = [0, Math.round(yMax / 2), yMax];
  const groupOffset = (plotW - bars.length * barW - (bars.length - 1) * gap) / 2;

  return (
    <div className="category-columns" role="img" aria-label="Findings by attack category">
      <svg
        className="category-columns__svg"
        viewBox={`0 0 ${CHART_W} ${CHART_H}`}
        preserveAspectRatio="xMidYMid meet"
      >
        <defs>
          {bars.map((item) => (
            <linearGradient key={`g-${item.id}`} id={`col-grad-${item.id}`} x1="0" y1="1" x2="0" y2="0">
              <stop offset="0%" stopColor={item.color} stopOpacity="0.55" />
              <stop offset="100%" stopColor={item.color} stopOpacity="1" />
            </linearGradient>
          ))}
        </defs>

        {ticks.map((tick) => {
          const y = PAD.top + plotH - (tick / yMax) * plotH;
          return (
            <g key={tick}>
              <line
                className="category-columns__grid"
                x1={PAD.left}
                x2={CHART_W - PAD.right}
                y1={y}
                y2={y}
              />
              <text className="category-columns__tick" x={PAD.left - 6} y={y + 3} textAnchor="end">
                {tick}
              </text>
            </g>
          );
        })}

        {bars.map((item, index) => {
          // Keep short columns visible when one category dominates.
          const h = Math.max((item.count / yMax) * plotH, 8);
          const x = PAD.left + groupOffset + index * (barW + gap);
          const y = PAD.top + plotH - h;
          return (
            <g
              key={item.id}
              className="category-columns__bar-group"
              style={{ animationDelay: `${index * 55}ms` }}
            >
              <title>{`${item.label}: ${item.count}`}</title>
              <rect
                className="category-columns__bar"
                x={x}
                y={y}
                width={barW}
                height={h}
                rx={5}
                ry={5}
                fill={`url(#col-grad-${item.id})`}
              />
              <text className="category-columns__value" x={x + barW / 2} y={y - 6} textAnchor="middle">
                {item.count}
              </text>
              <text
                className="category-columns__axis-label"
                x={x + barW / 2}
                y={CHART_H - 28}
                textAnchor="middle"
              >
                {shortLabel(item.label)}
              </text>
            </g>
          );
        })}
      </svg>

      <ul className="category-columns__legend">
        {bars.map((item) => (
          <li key={item.id} className="category-columns__legend-item">
            <span className="category-columns__swatch" style={{ background: item.color }} aria-hidden />
            <span className="category-columns__legend-label">{item.label}</span>
            <span className="category-columns__legend-count">{item.count}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
