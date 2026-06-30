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
  VARIANTS_PER_TEST_MAX,
  VARIANTS_PER_TEST_MIN,
  type PayloadStrategyConfig,
} from "../payloadStrategy";
import { WizardRangeSlider } from "./WizardRangeSlider";

type PayloadStrategySectionProps = {
  strategy: PayloadStrategyConfig;
  recommendedStrategy: PayloadStrategyConfig;
  onChange: (patch: Partial<PayloadStrategyConfig>) => void;
  onAcceptRecommended: () => void;
  readOnly?: boolean;
};

export function PayloadStrategySection({
  strategy,
  recommendedStrategy,
  onChange,
  onAcceptRecommended,
  readOnly = false,
}: PayloadStrategySectionProps) {
  const matchesRecommendation = payloadStrategyMatchesRecommendation(strategy, recommendedStrategy);

  const applyChange = (patch: Partial<PayloadStrategyConfig>) => {
    if (readOnly) return;
    onChange(patch);
  };

  return (
    <section className="wizard-fingerprint-summary">
      <div className="wizard-attack-categories__header">
        <h4 className="wizard-endpoints__title">Payload strategy</h4>
        {!readOnly && !matchesRecommendation && (
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
      {!readOnly && !matchesRecommendation && (
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
            onClick={() => applyChange({ strategy: item.id })}
            aria-pressed={strategy.strategy === item.id}
            aria-disabled={readOnly}
            data-readonly={readOnly || undefined}
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
            onClick={() => applyChange({ mutationLevel: item.id })}
            aria-pressed={strategy.mutationLevel === item.id}
            aria-disabled={readOnly}
            data-readonly={readOnly || undefined}
            title={item.tooltip}
          >
            <span className="wizard-attack-profile__label">{item.label}</span>
            <span className="wizard-attack-profile__description text-sm">{item.description}</span>
          </button>
        ))}
      </div>

      <div className="wizard-payload-sliders">
        <WizardRangeSlider
          label="Variants per test"
          value={strategy.variantsPerTest}
          min={VARIANTS_PER_TEST_MIN}
          max={VARIANTS_PER_TEST_MAX}
          formatValue={(value) => `${value}`}
          title="Maximum payload candidates per test (not HTTP requests)."
          onChange={(value) => applyChange({ variantsPerTest: clampVariantsPerTest(value) })}
          disabled={readOnly}
        />
        <WizardRangeSlider
          label="Maximum payload budget"
          value={strategy.maxTotalPayloads}
          min={PAYLOAD_BUDGET_MIN}
          max={PAYLOAD_BUDGET_MAX}
          step={PAYLOAD_BUDGET_STEP}
          formatValue={(value) => value.toLocaleString()}
          title="Upper bound on generated payloads. Execution may stop earlier."
          onChange={(value) => applyChange({ maxTotalPayloads: clampPayloadBudget(value) })}
          disabled={readOnly}
        />
      </div>

      <details className="wizard-fingerprint-summary wizard-advanced-options" open={readOnly}>
        <summary className="wizard-endpoints__title text-sm">Advanced options</summary>
        <div className="wizard-advanced-options__list">
          {ADVANCED_OPTIONS.map((option) => (
            <label key={option.key} className="wizard-checkbox" title={option.tooltip}>
              <input
                type="checkbox"
                checked={strategy[option.key]}
                disabled={readOnly}
                onChange={(event) => applyChange({ [option.key]: event.target.checked })}
              />
              <span>{option.label}</span>
            </label>
          ))}
        </div>
      </details>
    </section>
  );
}
