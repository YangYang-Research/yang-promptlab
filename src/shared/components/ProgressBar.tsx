type ProgressBarProps = {
  value: number;
  max?: number;
  label?: string;
  size?: "sm" | "md";
};

export function ProgressBar({ value, max = 100, label, size = "md" }: ProgressBarProps) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));
  return (
    <div className={`progress progress--${size}`}>
      {label && (
        <div className="progress__header">
          <span className="progress__label">{label}</span>
          <span className="progress__value">{Math.round(pct)}%</span>
        </div>
      )}
      <div className="progress__track" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
        <div className="progress__fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
