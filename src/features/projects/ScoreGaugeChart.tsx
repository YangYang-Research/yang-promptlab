import type { ProjectScoreTone } from "@/shared/stats";

type ScoreGaugeChartProps = {
  /** Score 0–100, or `null` when not yet available (N/A). */
  score: number | null;
  /** Overall gauge width in px. Height is derived for a semicircle. */
  size?: number;
  tone: ProjectScoreTone;
};

/**
 * Semicircle (½ circle) gauge for a 0–100 project health score.
 * Fill sweeps left → right as the score increases. Shows N/A when unscored.
 */
export function ScoreGaugeChart({ score, size = 160, tone }: ScoreGaugeChartProps) {
  const available = score != null;
  const clamped = available ? Math.max(0, Math.min(100, Math.round(score))) : 0;
  const strokeWidth = Math.max(9, size * 0.08);
  const pad = strokeWidth / 2 + 1;
  const width = size;
  const height = size * 0.49;
  const cx = width / 2;
  const cy = height - pad;
  const r = width / 2 - pad;
  const x1 = cx - r;
  const x2 = cx + r;
  const arc = `M ${x1} ${cy} A ${r} ${r} 0 0 1 ${x2} ${cy}`;

  return (
    <div
      className={`score-gauge score-gauge--${tone}`}
      aria-label={
        available
          ? `Project health score ${clamped} out of 100`
          : "Project health score not available"
      }
    >
      <div className="score-gauge__chart" style={{ width, height }}>
        <svg
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-hidden="true"
        >
          <path
            d={arc}
            fill="none"
            stroke="var(--bg-elevated)"
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            pathLength={100}
          />
          {available ? (
            <path
              className="score-gauge__arc"
              d={arc}
              fill="none"
              stroke="currentColor"
              strokeWidth={strokeWidth}
              strokeLinecap="round"
              pathLength={100}
              strokeDasharray={`${clamped} 100`}
            />
          ) : null}
        </svg>
        <div className="score-gauge__center">
          {available ? (
            <>
              <span className="score-gauge__value">{clamped}</span>
              <span className="score-gauge__max">/100</span>
            </>
          ) : (
            <span className="score-gauge__na">N/A</span>
          )}
        </div>
      </div>
      <div className="score-gauge__scale" aria-hidden="true">
        <span>0</span>
        <span>100</span>
      </div>
    </div>
  );
}
