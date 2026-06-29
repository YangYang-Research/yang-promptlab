import { Badge } from "@/shared/components";

import {
  ADVANCED_OPTIONS,
  clampPayloadBudget,
  clampVariantsPerTest,
  GENERATION_STRATEGIES,
  MUTATION_LEVELS,
  PAYLOAD_BUDGET_MAX,
  PAYLOAD_BUDGET_MIN,
  PAYLOAD_BUDGET_STEP,
  payloadStrategyMatchesRecommendation,
  sliderPercent,
  VARIANTS_PER_TEST_MAX,
  VARIANTS_PER_TEST_MIN,
  type PayloadStrategyConfig,
} from "../payloadStrategy";

type PayloadStrategySliderProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  formatValue: (value: number) => string;
  title?: string;
  onChange: (value: number) => void;
};

function PayloadStrategySlider({
  label,
  value,
  min,
  max,
  step = 1,
  formatValue,
  title,
  onChange,
}: PayloadStrategySliderProps) {
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

type PayloadStrategySectionProps = {
  strategy: PayloadStrategyConfig;
  recommendedStrategy: PayloadStrategyConfig;
  onChange: (patch: Partial<PayloadStrategyConfig>) => void;
  onAcceptRecommended: () => void;
};

export function PayloadStrategySection({
  strategy,
  recommendedStrategy,
  onChange,
  onAcceptRecommended,
}: PayloadStrategySectionProps) {
  const matchesRecommendation = payloadStrategyMatchesRecommendation(strategy, recommendedStrategy);

  return (
    <section className="wizard-fingerprint-summary">
      <div className="wizard-attack-categories__header">
        <h4 className="wizard-endpoints__title">Payload strategy</h4>
        {!matchesRecommendation && (
          <button
            type="button"
            className="wizard-attack-category__expand text-sm"
            onClick={onAcceptRecommended}
            title="Apply planner-recommended payload strategy"
          >
            Use recommended
          </button>
        )}
      </div>
      <p className="text-muted text-sm">
        Configures how payloads are generated during Step 5 execution. No probes are built here.
      </p>
      {!matchesRecommendation && (
        <p className="text-sm wizard-planner-summary">
          <Badge variant="info">Planner recommendation</Badge>{" "}
          {GENERATION_STRATEGIES.find((s) => s.id === recommendedStrategy.strategy)?.label} ·{" "}
          {MUTATION_LEVELS.find((m) => m.id === recommendedStrategy.mutationLevel)?.label} ·{" "}
          {recommendedStrategy.variantsPerTest} variants
        </p>
      )}

      <div className="wizard-attack-profiles">
        {GENERATION_STRATEGIES.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`wizard-attack-profile${strategy.strategy === item.id ? " wizard-attack-profile--selected" : ""}`}
            onClick={() => onChange({ strategy: item.id })}
            aria-pressed={strategy.strategy === item.id}
            title={item.tooltip}
          >
            <span className="wizard-attack-profile__label">{item.label}</span>
            <span className="wizard-attack-profile__description text-sm">{item.description}</span>
          </button>
        ))}
      </div>

      <div className="wizard-attack-profiles" style={{ marginTop: "0.75rem" }}>
        {MUTATION_LEVELS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`wizard-attack-profile${strategy.mutationLevel === item.id ? " wizard-attack-profile--selected" : ""}`}
            onClick={() => onChange({ mutationLevel: item.id })}
            aria-pressed={strategy.mutationLevel === item.id}
            title={item.tooltip}
          >
            <span className="wizard-attack-profile__label">{item.label}</span>
            <span className="wizard-attack-profile__description text-sm">{item.description}</span>
          </button>
        ))}
      </div>

      <div className="wizard-payload-sliders">
        <PayloadStrategySlider
          label="Variants per test"
          value={strategy.variantsPerTest}
          min={VARIANTS_PER_TEST_MIN}
          max={VARIANTS_PER_TEST_MAX}
          formatValue={(value) => `${value}`}
          title="Maximum payload candidates per test (not HTTP requests)."
          onChange={(value) => onChange({ variantsPerTest: clampVariantsPerTest(value) })}
        />
        <PayloadStrategySlider
          label="Maximum payload budget"
          value={strategy.maxTotalPayloads}
          min={PAYLOAD_BUDGET_MIN}
          max={PAYLOAD_BUDGET_MAX}
          step={PAYLOAD_BUDGET_STEP}
          formatValue={(value) => value.toLocaleString()}
          title="Upper bound on generated payloads. Execution may stop earlier."
          onChange={(value) => onChange({ maxTotalPayloads: clampPayloadBudget(value) })}
        />
      </div>

      <details className="wizard-fingerprint-summary" style={{ marginTop: "1rem" }}>
        <summary className="wizard-endpoints__title text-sm">Advanced options</summary>
        <div className="wizard-agent-options">
          {ADVANCED_OPTIONS.map((option) => (
            <label key={option.key} className="wizard-checkbox" title={option.tooltip}>
              <input
                type="checkbox"
                checked={strategy[option.key]}
                onChange={(event) => onChange({ [option.key]: event.target.checked })}
              />
              {option.label}
            </label>
          ))}
        </div>
      </details>
    </section>
  );
}
