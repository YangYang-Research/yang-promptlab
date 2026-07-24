import { Link } from "react-router-dom";

import {
  ADVANCED_OPTIONS,
  clampPayloadBudget,
  clampVariantsPerTest,
  GENERATION_STRATEGIES,
  MUTATION_LEVELS,
  PAYLOAD_BUDGET_MAX,
  PAYLOAD_BUDGET_MIN,
  PAYLOAD_BUDGET_STEP,
  VARIANTS_PER_TEST_MAX,
  VARIANTS_PER_TEST_MIN,
  type PayloadStrategyConfig,
} from "../payloadStrategy";
import { YazgBadge } from "@/shared/components";
import { WizardRangeSlider } from "./WizardRangeSlider";

type PayloadStrategySectionProps = {
  strategy: PayloadStrategyConfig;
  onChange: (patch: Partial<PayloadStrategyConfig>) => void;
  readOnly?: boolean;
};

export function PayloadStrategySection({
  strategy,
  onChange,
  readOnly = false,
}: PayloadStrategySectionProps) {
  const applyChange = (patch: Partial<PayloadStrategyConfig>) => {
    if (readOnly) return;
    onChange(patch);
  };

  return (
    <section className="wizard-fingerprint-summary">
      <div className="wizard-attack-categories__header">
        <h4 className="wizard-endpoints__title">Payload strategy</h4>
        {strategy.enableResponseAdaptation ? <YazgBadge /> : null}
      </div>
      <p className="text-muted text-sm">
        Configures Step 5 generation and attack expansion. Budget = payloads per
        testcase; variants = HTTP mutator expansions per payload. Estimated requests ≈
        testcases × budget × variants.
      </p>

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
          title="HTTP mutator expansions per generated payload (1 original + up to N−1 mutations). Multiplies request count."
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
          title="Generated payloads per testcase (technique). HTTP estimate ≈ testcases × budget × variants."
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

        <div className="wizard-mutator-config" style={{ marginTop: "1rem" }}>
          <div className="wizard-attack-categories__header">
            <h5 className="wizard-endpoints__title text-sm">Attack mutators</h5>
            {!readOnly ? (
              <Link to="/mutators" className="text-sm link">
                Manage in Advanced
              </Link>
            ) : null}
          </div>
          <p className="text-muted text-sm">
            Category assignments live under Advanced → Mutators. Variants per test still
            caps how many shapes run per payload.
          </p>
        </div>
      </details>
    </section>
  );
}
