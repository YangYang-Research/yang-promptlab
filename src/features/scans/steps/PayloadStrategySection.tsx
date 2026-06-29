import { Badge } from "@/shared/components";

import {
  ADVANCED_OPTIONS,
  GENERATION_STRATEGIES,
  MUTATION_LEVELS,
  payloadStrategyMatchesRecommendation,
  type PayloadStrategyConfig,
} from "../payloadStrategy";

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
          </button>
        ))}
      </div>

      <div className="wizard-agent-options" style={{ marginTop: "1rem" }}>
        <label className="text-sm" title="Maximum payload candidates per test (not HTTP requests).">
          Variants per test
          <input
            type="number"
            min={1}
            max={20}
            value={strategy.variantsPerTest}
            onChange={(event) =>
              onChange({
                variantsPerTest: Math.min(20, Math.max(1, Number(event.target.value) || 5)),
              })
            }
          />
        </label>
        <label
          className="text-sm"
          title="Upper bound on generated payloads. Execution may stop earlier."
        >
          Maximum payload budget
          <input
            type="number"
            min={50}
            max={5000}
            step={50}
            value={strategy.maxTotalPayloads}
            onChange={(event) =>
              onChange({
                maxTotalPayloads: Math.min(5000, Math.max(50, Number(event.target.value) || 500)),
              })
            }
          />
        </label>
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
