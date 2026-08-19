type ConfidenceMeterProps = {
  confidence: number;
};

function confidencePercent(confidence: number): number {
  return Math.round(confidence > 1 ? confidence : confidence * 100);
}

export function ConfidenceMeter({ confidence }: ConfidenceMeterProps) {
  const percent = Math.min(100, Math.max(0, confidencePercent(confidence)));

  return (
    <div className="confidence-meter" title={`${percent}%`}>
      <div className="confidence-meter__track">
        <div className="confidence-meter__bar" style={{ width: `${percent}%` }} />
      </div>
      <span className="confidence-meter__value">{percent}%</span>
    </div>
  );
}
