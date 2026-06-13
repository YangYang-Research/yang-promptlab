import { useEffect, useMemo } from "react";

import { Badge } from "@/shared/components";

import {
  ATTACK_CATALOG,
  ATTACK_PROFILES,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  getCategory,
  type AttackCategoryId,
  type AttackPlanConfig,
  type AttackProfileId,
} from "../attackProfiles";
import type { AttackPlanUiState } from "../wizardState";

type AttackPlanStepProps = {
  selectedEndpointCount: number;
  planUi: AttackPlanUiState;
  onPlanUiChange: (patch: Partial<AttackPlanUiState>) => void;
  onPlanChange?: (plan: AttackPlanConfig) => void;
};

export function AttackPlanStep({
  selectedEndpointCount,
  planUi,
  onPlanUiChange,
  onPlanChange,
}: AttackPlanStepProps) {
  const { profileId, customCategories, expandedCategory, disabledTests } = planUi;
  const disabledTestSet = useMemo(() => new Set(disabledTests), [disabledTests]);

  const activeCategories = useMemo(() => {
    if (profileId === "custom") return customCategories;
    return ATTACK_PROFILES.find((profile) => profile.id === profileId)?.categories ?? [];
  }, [profileId, customCategories]);

  const estimateInput = useMemo(
    () => ({
      selectedEndpointCount,
      profileId,
      customCategories,
      disabledTestIds: disabledTestSet,
    }),
    [selectedEndpointCount, profileId, customCategories, disabledTestSet],
  );

  const estimatedRequests = estimateRequests(estimateInput);
  const estimatedRuntime = formatEstimatedRuntime(estimateRuntimeSeconds(estimateInput));

  useEffect(() => {
    onPlanChange?.({
      profileId,
      customCategories,
      disabledTests,
      categories: activeCategories,
    });
  }, [profileId, customCategories, disabledTests, activeCategories, onPlanChange]);

  function selectProfile(next: AttackProfileId) {
    onPlanUiChange({
      profileId: next,
      disabledTests: next !== "custom" ? [] : disabledTests,
    });
  }

  function toggleCustomCategory(id: AttackCategoryId, enabled: boolean) {
    onPlanUiChange({
      customCategories: enabled
        ? customCategories.includes(id)
          ? customCategories
          : [...customCategories, id]
        : customCategories.filter((category) => category !== id),
    });
  }

  function toggleTest(testId: string, enabled: boolean) {
    const next = new Set(disabledTests);
    if (enabled) next.delete(testId);
    else next.add(testId);
    onPlanUiChange({
      profileId: profileId !== "custom" ? "custom" : profileId,
      disabledTests: [...next],
    });
  }

  return (
    <div className="wizard-step">
      <div className="wizard-attack-profiles">
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
      </div>

      <div className="wizard-attack-categories">
        <div className="wizard-attack-categories__header">
          <h4 className="wizard-endpoints__title">Attack categories</h4>
          <span className="text-muted text-sm">
            {activeCategories.length} of {ATTACK_CATALOG.length} selected
          </span>
        </div>

        <div className="wizard-attack-category-list">
          {ATTACK_CATALOG.map((category) => {
            const included =
              profileId === "custom"
                ? customCategories.includes(category.id)
                : activeCategories.includes(category.id);
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

                {expanded && included && (
                  <p className="text-muted text-sm wizard-attack-category__note">
                    {getCategory(category.id).description}
                  </p>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <div className="wizard-attack-estimates">
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Estimated requests</span>
          <span className="wizard-attack-estimate__value">
            {estimatedRequests.toLocaleString()}
          </span>
        </div>
        <div className="wizard-attack-estimate">
          <span className="wizard-attack-estimate__label">Estimated runtime</span>
          <span className="wizard-attack-estimate__value">{estimatedRuntime}</span>
        </div>
      </div>
    </div>
  );
}
