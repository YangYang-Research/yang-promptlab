import { useMemo, useRef } from "react";

import { Badge } from "@/shared/components";
import { adjustAttackPlan } from "@/shared/ipc/attackPlanner";
import { toAppError } from "@/shared/errors";

import {
  attackPlanFromDto,
  formatCoverageScore,
  formatEstimatedRuntime,
  payloadStrategyToDto,
  previewPlanForProfile,
  recomputePlanPreview,
  resolveCategoriesForAdjust,
  type AttackPlanConfig,
} from "../attackPlan";
import {
  ATTACK_CATALOG,
  ATTACK_PROFILES,
  getCategory,
  type AttackCategoryId,
  type AttackProfileId,
  type ExecutionStrategy,
} from "../attackProfiles";
import type { PayloadStrategyConfig } from "../payloadStrategy";
import { attackPlanUiFromPlan, type AttackPlanUiState } from "../wizardState";
import { PayloadStrategySection } from "./PayloadStrategySection";

type ReviewAttackPlanStepProps = {
  targetId: string;
  attackPlan: AttackPlanConfig;
  planUi: AttackPlanUiState;
  onPlanUiChange: (patch: Partial<AttackPlanUiState>) => void;
  onPlanChange: (plan: AttackPlanConfig) => void;
  onAdjustingChange?: (adjusting: boolean) => void;
};

export function ReviewAttackPlanStep({
  targetId,
  attackPlan,
  planUi,
  onPlanUiChange,
  onPlanChange,
  onAdjustingChange,
}: ReviewAttackPlanStepProps) {
  const { profileId, customCategories, expandedCategory, disabledTests, disabledGraphNodes } =
    planUi;
  const disabledTestSet = useMemo(() => new Set(disabledTests), [disabledTests]);
  const disabledGraphSet = useMemo(() => new Set(disabledGraphNodes), [disabledGraphNodes]);
  const payloadAdjustTimerRef = useRef<number | null>(null);
  const adjustRequestRef = useRef(0);

  const activeCategories = attackPlan.categories;

  async function applyAdjust(
    patch: Partial<AttackPlanUiState>,
    execution?: {
      executionStrategy?: ExecutionStrategy;
      maxAttempts?: number;
      reflectionEnabled?: boolean;
      adaptivePlanning?: boolean;
    },
    payloadStrategy?: PayloadStrategyConfig,
  ) {
    const nextUi = { ...planUi, ...patch };
    if (Object.keys(patch).length > 0) {
      onPlanUiChange(patch);
    }
    onAdjustingChange?.(true);
    const profileChanged =
      patch.profileId !== undefined && patch.profileId !== planUi.profileId;
    const strategyForRequest = profileChanged
      ? undefined
      : payloadStrategyToDto(payloadStrategy ?? attackPlan.payloadStrategy);
    const requestId = ++adjustRequestRef.current;

    try {
      const dto = await adjustAttackPlan({
        targetId,
        profileId: nextUi.profileId,
        categories: resolveCategoriesForAdjust(nextUi.profileId, nextUi, attackPlan),
        disabledTests: nextUi.disabledTests,
        disabledGraphNodes: nextUi.disabledGraphNodes,
        executionStrategy: execution?.executionStrategy ?? attackPlan.executionStrategy,
        maxAttempts: execution?.maxAttempts ?? attackPlan.maxAttempts,
        reflectionEnabled: execution?.reflectionEnabled ?? attackPlan.reflectionEnabled,
        adaptivePlanning: execution?.adaptivePlanning ?? attackPlan.adaptivePlanning,
        ...(strategyForRequest ? { payloadStrategy: strategyForRequest } : {}),
      });
      if (requestId !== adjustRequestRef.current) return;

      const plan = attackPlanFromDto(dto);
      onPlanChange(plan);
      onPlanUiChange({
        ...attackPlanUiFromPlan(plan),
        expandedCategory: nextUi.expandedCategory,
      });
    } catch (err) {
      console.error(toAppError(err).message);
    } finally {
      if (requestId === adjustRequestRef.current) {
        onAdjustingChange?.(false);
      }
    }
  }

  function selectProfile(next: AttackProfileId) {
    const patch: Partial<AttackPlanUiState> = {
      profileId: next,
      disabledTests: next !== "custom" ? [] : disabledTests,
      disabledGraphNodes: next !== "custom" ? [] : disabledGraphNodes,
      customCategories:
        next === "custom"
          ? attackPlan.suggestedCategories.filter((id) => !disabledGraphSet.has(id))
          : customCategories,
    };
    onPlanChange(
      previewPlanForProfile(attackPlan, next, patch.disabledTests ?? disabledTests),
    );
    onPlanUiChange(patch);
    void applyAdjust(patch);
  }

  function toggleGraphNode(category: AttackCategoryId, enabled: boolean) {
    const nextDisabled = new Set(disabledGraphNodes);
    if (enabled) nextDisabled.delete(category);
    else nextDisabled.add(category);
    const patch = {
      profileId: "custom" as const,
      disabledGraphNodes: [...nextDisabled],
      customCategories: attackPlan.suggestedCategories.filter((id) => !nextDisabled.has(id)),
    };
    onPlanChange(
      recomputePlanPreview({
        ...attackPlan,
        profileId: "custom",
        categories: patch.customCategories,
        attackGraph: attackPlan.attackGraph.map((node) => ({
          ...node,
          enabled: patch.customCategories.includes(node.category),
        })),
      }),
    );
    void applyAdjust(patch);
  }

  function toggleCustomCategory(id: AttackCategoryId, enabled: boolean) {
    const nextDisabled = new Set(disabledGraphNodes);
    if (enabled) nextDisabled.delete(id);
    else nextDisabled.add(id);
    const custom = attackPlan.suggestedCategories.filter((cat) => !nextDisabled.has(cat));
    void applyAdjust({
      profileId: "custom",
      customCategories: custom,
      disabledGraphNodes: [...nextDisabled],
    });
  }

  function toggleTest(testId: string, enabled: boolean) {
    const next = new Set(disabledTests);
    if (enabled) next.delete(testId);
    else next.add(testId);
    void applyAdjust({
      profileId: profileId !== "custom" ? "custom" : profileId,
      customCategories: attackPlan.categories,
      disabledTests: [...next],
    });
  }

  function updateExecution(patch: {
    executionStrategy?: ExecutionStrategy;
    maxAttempts?: number;
    reflectionEnabled?: boolean;
    adaptivePlanning?: boolean;
  }) {
    onPlanChange(recomputePlanPreview({ ...attackPlan, ...patch }));
    void applyAdjust({}, patch);
  }

  function updatePayloadStrategy(patch: Partial<PayloadStrategyConfig>) {
    const nextStrategy = { ...attackPlan.payloadStrategy, ...patch };
    onPlanChange(recomputePlanPreview({ ...attackPlan, payloadStrategy: nextStrategy }));
    if (payloadAdjustTimerRef.current) {
      window.clearTimeout(payloadAdjustTimerRef.current);
    }
    payloadAdjustTimerRef.current = window.setTimeout(() => {
      void applyAdjust({}, undefined, nextStrategy);
    }, 300);
  }

  function acceptRecommendedPayloadStrategy() {
    void applyAdjust({}, undefined, attackPlan.recommendedPayloadStrategy);
  }

  const enabledGraph = attackPlan.attackGraph.filter((node) => node.enabled);

  return (
    <div className="wizard-step">
      <section className="wizard-fingerprint-summary">
        <h4 className="wizard-endpoints__title">Planner summary</h4>
        <dl className="wizard-attack-estimates">
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Total testcases</span>
            <span className="wizard-attack-estimate__value">
              {attackPlan.totalTestcases.toLocaleString()}
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Confidence</span>
            <span className="wizard-attack-estimate__value">
              {Math.round(attackPlan.confidence * 100)}%
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Est. runtime</span>
            <span className="wizard-attack-estimate__value">
              {formatEstimatedRuntime(attackPlan.estimatedRuntimeSeconds)}
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Est. requests</span>
            <span className="wizard-attack-estimate__value">
              {attackPlan.estimatedRequests.toLocaleString()}
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Est. tokens</span>
            <span className="wizard-attack-estimate__value">
              {attackPlan.estimatedTokens.toLocaleString()}
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Coverage</span>
            <span className="wizard-attack-estimate__value">
              {formatCoverageScore(attackPlan.coverageScore)}
            </span>
          </div>
        </dl>
        <p className="text-sm wizard-planner-summary">
          <strong>Summary:</strong> {attackPlan.summary}
        </p>
        {attackPlan.rationales.length > 0 && (
          <ul className="wizard-planner-rationales text-sm text-muted">
            {attackPlan.rationales.slice(0, 8).map((item) => (
              <li key={`${item.category}-${item.source}`}>
                {getCategory(item.category).label}: {item.reason}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="wizard-fingerprint-summary">
        <h4 className="wizard-endpoints__title">Attack graph</h4>
        <p className="text-muted text-sm">
          Execution order suggested by the planner. Disable nodes to exclude categories.
        </p>
        <ol className="wizard-attack-graph">
          {attackPlan.attackGraph.map((node, index) => {
            const included = activeCategories.includes(node.category);
            return (
              <li key={node.category} className={`wizard-attack-graph__node${included ? "" : " wizard-attack-graph__node--off"}`}>
                <div className="wizard-attack-graph__row">
                  <label className="wizard-attack-category__toggle">
                    <input
                      type="checkbox"
                      checked={included}
                      onChange={(e) => toggleGraphNode(node.category, e.target.checked)}
                    />
                    <span>{getCategory(node.category).label}</span>
                  </label>
                  {index < attackPlan.attackGraph.length - 1 && (
                    <span className="wizard-attack-graph__arrow" aria-hidden>
                      ↓
                    </span>
                  )}
                </div>
                <div className="wizard-attack-graph__meta text-sm text-muted">
                  Priority {node.priority} · Risk {node.risk} · Confidence{" "}
                  {Math.round(node.confidence * 100)}%
                  {node.dependencies.length > 0 && (
                    <> · Depends on {node.dependencies.map((d) => getCategory(d).label).join(", ")}</>
                  )}
                </div>
              </li>
            );
          })}
        </ol>
        {enabledGraph.length === 0 && (
          <p className="text-danger text-sm">Enable at least one attack category to continue.</p>
        )}
      </section>

      <section className="wizard-attack-profiles">
        {ATTACK_PROFILES.map((profile) => {
          const selected = profileId === profile.id;
          return (
            <button
              key={profile.id}
              type="button"
              className={`wizard-attack-profile${selected ? " wizard-attack-profile--selected" : ""}`}
              onClick={() => selectProfile(profile.id)}
              aria-pressed={selected}
            >
              <span className="wizard-attack-profile__label">{profile.label}</span>
              <span className="wizard-attack-profile__meta text-muted text-sm">
                {profile.id === "custom"
                  ? "Manual selection"
                  : `${profile.categories.length} categories`}
              </span>
              <span className="wizard-attack-profile__description text-sm">
                {profile.description}
              </span>
            </button>
          );
        })}
      </section>

      <section className="wizard-attack-categories">
        <div className="wizard-attack-categories__header">
          <h4 className="wizard-endpoints__title">Attack categories</h4>
          <span className="text-muted text-sm">
            {activeCategories.length} of {ATTACK_CATALOG.length} selected
          </span>
        </div>
        <div className="wizard-attack-category-list">
          {ATTACK_CATALOG.filter((cat) => attackPlan.suggestedCategories.includes(cat.id)).map(
            (category) => {
              const included = activeCategories.includes(category.id);
              const expanded = expandedCategory === category.id;
              const enabledTests = category.tests.filter((test) => !disabledTestSet.has(test.id));

              return (
                <div
                  key={category.id}
                  className={`wizard-attack-category${included ? "" : " wizard-attack-category--off"}`}
                >
                  <div className="wizard-attack-category__row">
                    {profileId === "custom" ? (
                      <label className="wizard-attack-category__toggle">
                        <input
                          type="checkbox"
                          checked={included}
                          onChange={(e) => toggleCustomCategory(category.id, e.target.checked)}
                        />
                        <span>{category.label}</span>
                      </label>
                    ) : (
                      <div className="wizard-attack-category__title">
                        <span>{category.label}</span>
                        {!included && <Badge variant="muted">Excluded</Badge>}
                      </div>
                    )}
                    <button
                      type="button"
                      className="wizard-attack-category__expand text-sm"
                      onClick={() =>
                        onPlanUiChange({
                          expandedCategory: expanded ? null : category.id,
                        })
                      }
                      aria-expanded={expanded}
                    >
                      {enabledTests.length}/{category.tests.length} tests
                    </button>
                  </div>

                  {expanded && (
                    <ul className="wizard-attack-test-list">
                      {category.tests.map((test) => {
                        const enabled = !disabledTestSet.has(test.id);
                        return (
                          <li key={test.id}>
                            <label className="wizard-attack-test">
                              <input
                                type="checkbox"
                                checked={enabled && included}
                                disabled={!included}
                                onChange={(e) => toggleTest(test.id, e.target.checked)}
                              />
                              <span>{test.name}</span>
                            </label>
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </div>
              );
            },
          )}
        </div>
      </section>

      <section className="wizard-fingerprint-summary">
        <h4 className="wizard-endpoints__title">Execution strategy</h4>
        <div className="wizard-attack-profiles">
          <button
            type="button"
            className={`wizard-attack-profile${attackPlan.executionStrategy === "sequential" ? " wizard-attack-profile--selected" : ""}`}
            onClick={() => updateExecution({ executionStrategy: "sequential" })}
            aria-pressed={attackPlan.executionStrategy === "sequential"}
          >
            <span className="wizard-attack-profile__label">Sequential</span>
            <span className="wizard-attack-profile__description text-sm">
              One execution pass through the approved attack graph.
            </span>
          </button>
          <button
            type="button"
            className={`wizard-attack-profile${attackPlan.executionStrategy === "agentic" ? " wizard-attack-profile--selected" : ""}`}
            onClick={() => updateExecution({ executionStrategy: "agentic" })}
            aria-pressed={attackPlan.executionStrategy === "agentic"}
          >
            <span className="wizard-attack-profile__label">Agentic</span>
            <span className="wizard-attack-profile__description text-sm">
              Planner → Attack → Judge → Reflection → Retry until stop condition.
            </span>
          </button>
        </div>
        {attackPlan.executionStrategy === "agentic" && (
          <div className="wizard-agent-options">
            <label className="text-sm">
              Maximum attempts per category
              <input
                type="number"
                min={1}
                max={20}
                value={attackPlan.maxAttempts}
                onChange={(event) =>
                  updateExecution({
                    maxAttempts: Math.min(20, Math.max(1, Number(event.target.value) || 5)),
                  })
                }
              />
            </label>
            <label className="wizard-checkbox">
              <input
                type="checkbox"
                checked={attackPlan.reflectionEnabled}
                onChange={(event) =>
                  updateExecution({ reflectionEnabled: event.target.checked })
                }
              />
              Reflection enabled
            </label>
            <label className="wizard-checkbox">
              <input
                type="checkbox"
                checked={attackPlan.adaptivePlanning}
                onChange={(event) =>
                  updateExecution({ adaptivePlanning: event.target.checked })
                }
              />
              Adaptive planning
            </label>
          </div>
        )}
      </section>

      <PayloadStrategySection
        strategy={attackPlan.payloadStrategy}
        recommendedStrategy={attackPlan.recommendedPayloadStrategy}
        onChange={updatePayloadStrategy}
        onAcceptRecommended={acceptRecommendedPayloadStrategy}
      />

      <section className="wizard-attack-estimates">
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Risk coverage</span>
          <span className="wizard-attack-estimate__value">
            {formatCoverageScore(attackPlan.riskCoverage)}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Est. requests</span>
          <span className="wizard-attack-estimate__value">
            {attackPlan.estimatedRequests.toLocaleString()}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Est. runtime</span>
          <span className="wizard-attack-estimate__value">
            {formatEstimatedRuntime(attackPlan.estimatedRuntimeSeconds)}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Est. tokens</span>
          <span className="wizard-attack-estimate__value">
            {attackPlan.estimatedTokens.toLocaleString()}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Coverage score</span>
          <span className="wizard-attack-estimate__value">
            {formatCoverageScore(attackPlan.coverageScore)}
          </span>
        </div>
      </section>
    </div>
  );
}
