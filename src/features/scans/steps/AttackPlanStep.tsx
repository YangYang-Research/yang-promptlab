import { useEffect, useMemo, useState } from "react";

import { Badge } from "@/shared/components";

import {
  ALL_ATTACK_CATEGORY_IDS,
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

type AttackPlanStepProps = {
  selectedEndpointCount: number;
  onPlanChange?: (plan: AttackPlanConfig) => void;
};

export function AttackPlanStep({ selectedEndpointCount, onPlanChange }: AttackPlanStepProps) {
  const [profileId, setProfileId] = useState<AttackProfileId>("standard");
  const [customCategories, setCustomCategories] = useState<AttackCategoryId[]>(
    ALL_ATTACK_CATEGORY_IDS,
  );
  const [expandedCategory, setExpandedCategory] = useState<AttackCategoryId | null>(null);
  const [disabledTests, setDisabledTests] = useState<Set<string>>(new Set());

  const activeCategories = useMemo(() => {
    if (profileId === "custom") return customCategories;
    return ATTACK_PROFILES.find((p) => p.id === profileId)?.categories ?? [];
  }, [profileId, customCategories]);

  const estimateInput = useMemo(
    () => ({
      selectedEndpointCount,
      profileId,
      customCategories,
      disabledTestIds: disabledTests,
    }),
    [selectedEndpointCount, profileId, customCategories, disabledTests],
  );

  const estimatedRequests = estimateRequests(estimateInput);
  const estimatedRuntime = formatEstimatedRuntime(estimateRuntimeSeconds(estimateInput));

  useEffect(() => {
    onPlanChange?.({
      profileId,
      customCategories,
      disabledTests: [...disabledTests],
      categories: activeCategories,
    });
  }, [profileId, customCategories, disabledTests, activeCategories, onPlanChange]);

  function selectProfile(next: AttackProfileId) {
    setProfileId(next);
    if (next !== "custom") {
      setDisabledTests(new Set());
    }
  }

  function toggleCustomCategory(id: AttackCategoryId, enabled: boolean) {
    setCustomCategories((prev) => {
      if (enabled) return prev.includes(id) ? prev : [...prev, id];
      return prev.filter((c) => c !== id);
    });
  }

  function toggleTest(testId: string, enabled: boolean) {
    setDisabledTests((prev) => {
      const next = new Set(prev);
      if (enabled) next.delete(testId);
      else next.add(testId);
      return next;
    });
    if (profileId !== "custom") {
      setProfileId("custom");
    }
  }

  return (
    <div className="wizard-step">
      <div className="wizard-step__heading">
        <span className="wizard-step__number">4</span>
        <div>
          <h3 className="wizard-step__title">Attack planning</h3>
          <p className="wizard-step__hint text-muted">
            Choose a profile for {selectedEndpointCount} selected endpoint
            {selectedEndpointCount === 1 ? "" : "s"}
          </p>
        </div>
      </div>

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
            const enabledTests = category.tests.filter((t) => !disabledTests.has(t.id));

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
                      setExpandedCategory(expanded ? null : category.id)
                    }
                    aria-expanded={expanded}
                  >
                    {enabledTests.length}/{category.tests.length} tests
                  </button>
                </div>

                {expanded && (
                  <ul className="wizard-attack-test-list">
                    {category.tests.map((test) => {
                      const enabled = !disabledTests.has(test.id);
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
