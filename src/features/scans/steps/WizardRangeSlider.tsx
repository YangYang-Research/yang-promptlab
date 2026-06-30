import { sliderPercent } from "../payloadStrategy";

type WizardRangeSliderProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  formatValue: (value: number) => string;
  title?: string;
  disabled?: boolean;
  onChange: (value: number) => void;
};

export function WizardRangeSlider({
  label,
  value,
  min,
  max,
  step = 1,
  formatValue,
  title,
  disabled = false,
  onChange,
}: WizardRangeSliderProps) {
  const fillPct = sliderPercent(value, min, max);

  return (
    <div className="wizard-payload-slider" title={title}>
      <div className="wizard-payload-slider__header">
        <span className="wizard-payload-slider__label">{label}</span>
        <span className="wizard-payload-slider__value">{formatValue(value)}</span>
      </div>
      <div className="wizard-payload-slider__track-wrap">
        <div className="progress__track" aria-hidden>
          <div className="progress__fill" style={{ width: `${fillPct}%` }} />
        </div>
        <input
          type="range"
          className="wizard-payload-slider__input"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onChange={(event) => onChange(Number(event.target.value))}
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={value}
          aria-label={label}
        />
      </div>
    </div>
  );
}
