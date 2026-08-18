import { useEffect, useMemo, useRef } from "react";

import { Badge } from "@/shared/components";
import { IconAi, IconHuman } from "@/shared/components/Icons";
import { YazgBadge } from "@/shared/components/YazgBadge";
import { adjustAttackPlan } from "@/shared/ipc/attackPlanner";
import { toAppError } from "@/shared/errors";

import {
  attackPlanFromDto,
  formatCoverageScore,
  formatEstimatedRuntime,
  formatExecutionStrategySummary,
  extractPlannerEndpoint,
  payloadStrategyToDto,
  planCustomizationKey,
  getProfileMode,
  plannerAdjustContext,
  plannerSourceFromPlan,
  planUiForCustomFromCategories,
  previewPlanForProfile,
  recomputePlanPreview,
  resolveCategoriesForAdjust,
  resolveActivePlannerRationales,
  syncAttackPlanUiAfterAdjust,
  type AttackPlanConfig,
} from "../attackPlan";
import {
  ALL_ATTACK_CATEGORY_IDS,
  ATTACK_CATALOG,
  ATTACK_PROFILES,
  getCategory,
  getProfile,
  type AttackCategoryId,
  type AttackProfileId,
  type ExecutionStrategy,
} from "../attackProfiles";
import type { PayloadStrategyConfig } from "../payloadStrategy";
import { formatPayloadGenerationStrategy } from "../payloadStrategy";
import type { AttackPlanUiState } from "../wizardState";
import { PayloadStrategySection } from "./PayloadStrategySection";
import { WizardRangeSlider } from "./WizardRangeSlider";

const MAX_ATTEMPTS_MIN = 1;
const MAX_ATTEMPTS_MAX = 20;
const TEST_COLUMN_COUNT = 2;

/** Split items into at most `maxColumns` columns (fill down, then next column). */
function chunkIntoColumns<T>(items: T[], maxColumns: number): T[][] {
  if (items.length === 0) return [];
  const columnCount = Math.min(Math.max(1, maxColumns), items.length);
  const perColumn = Math.ceil(items.length / columnCount);
  const columns: T[][] = [];
  for (let i = 0; i < items.length; i += perColumn) {
    columns.push(items.slice(i, i + perColumn));
  }
  return columns;
}

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
  const payloadAdjustTimerRef = useRef<number | null>(null);
  const adjustRequestRef = useRef(0);

  const activeCategories = attackPlan.categories;
  const isCustomProfile = profileId === "custom";
  const suggestedSet = useMemo(
    () => new Set(attackPlan.suggestedCategories),
    [attackPlan.suggestedCategories],
  );
  const notApplicableCount = ATTACK_CATALOG.length - attackPlan.suggestedCategories.length;
  const activeRationales = useMemo(
    () => resolveActivePlannerRationales(attackPlan, activeCategories),
    [attackPlan, activeCategories],
  );

  useEffect(() => {
    if (planUi.suggestedPlanKey !== null) return;
    onPlanUiChange({
      plannerSource: plannerSourceFromPlan(attackPlan),
      suggestedPlanKey: planCustomizationKey(attackPlan),
    });
  }, [attackPlan, onPlanUiChange, planUi.suggestedPlanKey]);

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
    const switchingPreset =
      profileChanged && patch.profileId !== "custom" && planUi.profileId !== "custom";
    const strategyForRequest = switchingPreset
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
        ...plannerAdjustContext(attackPlan),
      });
      if (requestId !== adjustRequestRef.current) return;

      const plan = attackPlanFromDto(dto);
      onPlanChange(plan);
      onPlanUiChange(syncAttackPlanUiAfterAdjust(plan, nextUi));
    } catch (err) {
      console.error(toAppError(err).message);
    } finally {
      if (requestId === adjustRequestRef.current) {
        onAdjustingChange?.(false);
      }
    }
  }

  function selectProfile(next: AttackProfileId) {
    const switchingToCustom = next === "custom" && profileId !== "custom";
    const presetCategories = switchingToCustom
      ? [...attackPlan.categories]
      : attackPlan.categories;
    const customUi = switchingToCustom
      ? planUiForCustomFromCategories(presetCategories)
      : null;
    const nextMode = next !== "custom" ? getProfileMode(attackPlan, next) : null;

    const patch: Partial<AttackPlanUiState> = {
      profileId: next,
      disabledTests:
        next !== "custom" ? (nextMode?.disabledTests ?? []) : disabledTests,
      disabledGraphNodes: switchingToCustom
        ? customUi!.disabledGraphNodes
        : next !== "custom"
          ? []
          : disabledGraphNodes,
      customCategories: switchingToCustom
        ? customUi!.customCategories
        : next === "custom"
          ? customCategories
          : customCategories,
    };
    const preview = previewPlanForProfile(attackPlan, next, patch.disabledTests ?? disabledTests, {
      sourceProfileId: switchingToCustom ? profileId : undefined,
    });
    onPlanChange(preview);
    onPlanUiChange(patch);
    void applyAdjust(
      patch,
      {
        executionStrategy: preview.executionStrategy,
        maxAttempts: preview.maxAttempts,
        reflectionEnabled: preview.reflectionEnabled,
        adaptivePlanning: preview.adaptivePlanning,
      },
      preview.payloadStrategy,
    );
  }

  function toggleCustomCategory(id: AttackCategoryId, enabled: boolean) {
    // Derive from the active selection — never from a partial disabledGraphNodes
    // list (attackGraph omits N/A categories, which previously auto-enabled them).
    const active = new Set(
      customCategories.length > 0 ? customCategories : attackPlan.categories,
    );
    if (enabled) active.add(id);
    else active.delete(id);
    const customUi = planUiForCustomFromCategories(
      ALL_ATTACK_CATEGORY_IDS.filter((cat) => active.has(cat)),
    );
    void applyAdjust({
      profileId: "custom",
      customCategories: customUi.customCategories,
      disabledGraphNodes: customUi.disabledGraphNodes,
    });
  }

  function toggleTest(testId: string, enabled: boolean) {
    const next = new Set(disabledTests);
    if (enabled) next.delete(testId);
    else next.add(testId);
    const promoting = profileId !== "custom";
    const customUi = promoting
      ? planUiForCustomFromCategories(attackPlan.categories)
      : null;
    void applyAdjust({
      profileId: promoting ? "custom" : profileId,
      customCategories: customUi?.customCategories ?? customCategories,
      disabledGraphNodes: customUi?.disabledGraphNodes ?? disabledGraphNodes,
      disabledTests: [...next],
    });
  }

  /** Editing execution/payload on a preset mirrors category edits: promote to Custom. */
  function promoteToCustomUi(): Partial<AttackPlanUiState> {
    if (profileId === "custom") return {};
    const customUi = planUiForCustomFromCategories(attackPlan.categories);
    return {
      profileId: "custom",
      customCategories: customUi.customCategories,
      disabledGraphNodes: customUi.disabledGraphNodes,
    };
  }

  function updateExecution(patch: {
    executionStrategy?: ExecutionStrategy;
    maxAttempts?: number;
    reflectionEnabled?: boolean;
    adaptivePlanning?: boolean;
  }) {
    const uiPatch = promoteToCustomUi();
    if (Object.keys(uiPatch).length > 0) {
      onPlanUiChange(uiPatch);
    }
    onPlanChange(
      recomputePlanPreview({
        ...attackPlan,
        ...(uiPatch.profileId === "custom"
          ? {
              profileId: "custom" as const,
              customCategories: uiPatch.customCategories!,
              disabledGraphNodes: uiPatch.disabledGraphNodes!,
            }
          : {}),
        ...patch,
      }),
    );
    void applyAdjust(uiPatch, patch);
  }

  function updatePayloadStrategy(patch: Partial<PayloadStrategyConfig>) {
    const uiPatch = promoteToCustomUi();
    if (Object.keys(uiPatch).length > 0) {
      onPlanUiChange(uiPatch);
    }
    const nextStrategy = { ...attackPlan.payloadStrategy, ...patch };
    onPlanChange(
      recomputePlanPreview({
        ...attackPlan,
        ...(uiPatch.profileId === "custom"
          ? {
              profileId: "custom" as const,
              customCategories: uiPatch.customCategories!,
              disabledGraphNodes: uiPatch.disabledGraphNodes!,
            }
          : {}),
        payloadStrategy: nextStrategy,
      }),
    );
    if (payloadAdjustTimerRef.current) {
      window.clearTimeout(payloadAdjustTimerRef.current);
    }
    payloadAdjustTimerRef.current = window.setTimeout(() => {
      void applyAdjust(uiPatch, undefined, nextStrategy);
    }, 300);
  }

  function profileModeBadge(profile: AttackProfileId) {
    if (profile === "custom") {
      return (
        <IconHuman
          className="wizard-attack-profile__mode-icon wizard-attack-profile__mode-icon--human"
          aria-label="Manual selection"
        />
      );
    }
    return (
      <IconAi
        className="wizard-attack-profile__mode-icon"
        aria-label={
          attackPlan.recommendedProfileId === profile ? "Recommended AI plan" : "AI planned"
        }
      />
    );
  }

  function profileModeMeta(id: AttackProfileId): string {
    if (id === "custom") return "Manual selection";
    const mode = getProfileMode(attackPlan, id);
    if (!mode) return "Re-plan with Yazg";
    return `${mode.categories.length} categories · ${formatExecutionStrategySummary(mode)} · ${formatPayloadGenerationStrategy(mode.payloadStrategy)}`;
  }

  return (
    <div className="wizard-step">
      <section className="wizard-fingerprint-summary">
        <div className="wizard-planner-summary-header">
          <h4 className="wizard-endpoints__title">Planner summary</h4>
        </div>
        <dl className="wizard-attack-estimates">
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Attack mode</span>
            <span className="wizard-attack-estimate__value">{getProfile(profileId).label}</span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Active tests</span>
            <span className="wizard-attack-estimate__value">
              {attackPlan.totalTestcases.toLocaleString()}
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Payload strategy</span>
            <span className="wizard-attack-estimate__value">
              {formatPayloadGenerationStrategy(attackPlan.payloadStrategy)}
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
            <span className="wizard-attack-estimate__label">Execution strategy</span>
            <span className="wizard-attack-estimate__value">
              {formatExecutionStrategySummary(attackPlan)}
            </span>
          </div>
          <div className="wizard-attack-estimate">
            <span className="wizard-attack-estimate__label">Est. runtime</span>
            <span className="wizard-attack-estimate__value">
              {formatEstimatedRuntime(attackPlan.estimatedRuntimeSeconds)}
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
          <strong>Plan for:</strong>{" "}
          <span className="wizard-planner-summary__url mono">
            {extractPlannerEndpoint(attackPlan.summary)}
          </span>
        </p>
        {activeRationales.length > 0 && (
          <ul className="wizard-planner-rationales text-sm text-muted">
            {activeRationales.map((item) => (
              <li key={`${item.category}-${item.source}`}>
                {getCategory(item.category).label}: {item.reason}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="wizard-fingerprint-summary">
        <div className="wizard-planner-summary-header">
          <h4 className="wizard-endpoints__title">Attack Mode</h4>
        </div>
        <div className="wizard-attack-profiles">
        {ATTACK_PROFILES.map((profile) => {
          const selected = profileId === profile.id;
          const recommended = attackPlan.recommendedProfileId === profile.id;
          return (
            <button
              key={profile.id}
              type="button"
              className={[
                "wizard-attack-profile",
                selected ? "wizard-attack-profile--selected" : "",
                recommended ? "wizard-attack-profile--recommended" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              onClick={() => selectProfile(profile.id)}
              aria-pressed={selected}
            >
              {recommended ? (
                <YazgBadge
                  label="Recommended"
                  className="wizard-attack-profile__yazg-badge"
                />
              ) : null}
              <div className="wizard-attack-profile__top">
                <span className="wizard-attack-profile__label">{profile.label}</span>
                {profileModeBadge(profile.id)}
              </div>
              <span className="wizard-attack-profile__meta text-muted text-sm">
                {profileModeMeta(profile.id)}
              </span>
              <span className="wizard-attack-profile__description text-sm">
                {profile.id === "custom"
                  ? profile.description
                  : getProfileMode(attackPlan, profile.id)?.description?.trim() ||
                    profile.description}
              </span>
            </button>
          );
        })}
        </div>
      </section>

      <section className="wizard-attack-categories">
        <div className="wizard-attack-categories__header">
          <h4 className="wizard-endpoints__title">Attack categories</h4>
          <span className="text-muted text-sm">
            {activeCategories.length} of{" "}
            {isCustomProfile ? ATTACK_CATALOG.length : attackPlan.suggestedCategories.length}{" "}
            {isCustomProfile ? "categories" : "applicable"} selected
            {!isCustomProfile && notApplicableCount > 0
              ? ` · ${notApplicableCount} not applicable`
              : ""}
          </span>
        </div>
        <div className="wizard-attack-category-list">
          {ATTACK_CATALOG.map((category) => {
              const applicable = isCustomProfile || suggestedSet.has(category.id);
              if (!applicable) {
                return (
                  <div
                    key={category.id}
                    className="wizard-attack-category wizard-attack-category--na"
                  >
                    <div className="wizard-attack-category__row">
                      <div className="wizard-attack-category__title">
                        <span>{category.label}</span>
                        <Badge variant="muted">Not applicable</Badge>
                      </div>
                    </div>
                    <p className="wizard-attack-category__note text-sm text-muted">
                      {category.description} — not suggested for this target based on its
                      capabilities.
                    </p>
                  </div>
                );
              }

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
                    <div className="wizard-attack-test-columns">
                      {chunkIntoColumns(category.tests, TEST_COLUMN_COUNT).map((column, colIndex) => (
                        <ul key={colIndex} className="wizard-attack-test-list">
                          {column.map((test) => {
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
                      ))}
                    </div>
                  )}
                </div>
              );
            },
          )}
        </div>
      </section>

      <section className="wizard-fingerprint-summary">
        <div className="wizard-planner-summary-header">
          <h4 className="wizard-endpoints__title">Execution strategy</h4>
        </div>
        <div className="wizard-attack-profiles">
          <button
            type="button"
            className={`wizard-attack-profile${attackPlan.executionStrategy === "sequential" ? " wizard-attack-profile--selected" : ""}`}
            onClick={() => updateExecution({ executionStrategy: "sequential" })}
            aria-pressed={attackPlan.executionStrategy === "sequential"}
          >
            <span className="wizard-attack-profile__label">Sequential</span>
            <span className="wizard-attack-profile__description text-sm">
              One execution pass through the selected attack categories.
            </span>
          </button>
          <button
            type="button"
            className={`wizard-attack-profile${attackPlan.executionStrategy === "agentic" ? " wizard-attack-profile--selected" : ""}`}
            onClick={() =>
              updateExecution({
                executionStrategy: "agentic",
                reflectionEnabled: true,
              })
            }
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
            <div className="wizard-payload-sliders">
              <WizardRangeSlider
                label="Maximum attempts per category"
                value={attackPlan.maxAttempts}
                min={MAX_ATTEMPTS_MIN}
                max={MAX_ATTEMPTS_MAX}
                formatValue={(value) => `${value}`}
                title="Maximum agentic retry attempts per attack category."
                onChange={(value) =>
                  updateExecution({
                    maxAttempts: Math.min(
                      MAX_ATTEMPTS_MAX,
                      Math.max(MAX_ATTEMPTS_MIN, value),
                    ),
                  })
                }
              />
            </div>
            <div className="wizard-agent-options__checks">
              <label className="wizard-checkbox">
                <input
                  type="checkbox"
                  checked={attackPlan.reflectionEnabled}
                  onChange={(event) =>
                    updateExecution({ reflectionEnabled: event.target.checked })
                  }
                />
                <span>Reflection enabled</span>
              </label>
              <label
                className="wizard-checkbox"
                title="Between retries, rotate techniques and escalate mutation/strategy from judge outcomes."
              >
                <input
                  type="checkbox"
                  checked={attackPlan.adaptivePlanning}
                  onChange={(event) =>
                    updateExecution({ adaptivePlanning: event.target.checked })
                  }
                />
                <span>Adaptive planning</span>
              </label>
            </div>
          </div>
        )}
      </section>

      <PayloadStrategySection
        strategy={attackPlan.payloadStrategy}
        onChange={updatePayloadStrategy}
      />
    </div>
  );
}
