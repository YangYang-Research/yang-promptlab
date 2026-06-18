import { useEffect, useMemo, useState } from "react";

import { Badge, Button } from "@/shared/components";
import type { EndpointDto } from "@/shared/ipc";
import { generateAttackPlan, type AttackPlanDto } from "@/shared/ipc/planner";
import { generatePromptPayloads, type PromptPayloadsDto } from "@/shared/ipc/generator";
import { toAppError } from "@/shared/errors";

import {
  ATTACK_CATALOG,
  ATTACK_PROFILES,
  ALL_ATTACK_CATEGORY_IDS,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  getCategory,
  type AttackCategoryId,
  type AttackPlanConfig,
  type AttackProfileId,
  type GeneratorMode,
} from "../attackProfiles";
import {
  aggregateAttackSuggestions,
  aggregatePlatformSummary,
  platformLabel,
} from "../fingerprintPlan";
import type { AttackPlanUiState } from "../wizardState";

type AttackPlanStepProps = {
  selectedEndpointCount: number;
  endpoints: EndpointDto[];
  selectedEndpointIds: string[];
  planUi: AttackPlanUiState;
  onPlanUiChange: (patch: Partial<AttackPlanUiState>) => void;
  onPlanChange?: (plan: AttackPlanConfig) => void;
};

export function AttackPlanStep({
  selectedEndpointCount,
  endpoints,
  selectedEndpointIds,
  planUi,
  onPlanUiChange,
  onPlanChange,
}: AttackPlanStepProps) {
  const { profileId, customCategories, expandedCategory, disabledTests, plannerSummary, plannerMode, generatorMode, generatorSummary, agentMode, maxAgentAttempts } =
    planUi;
  const disabledTestSet = useMemo(() => new Set(disabledTests), [disabledTests]);
  const [generating, setGenerating] = useState<"deterministic" | "local_llm" | null>(null);
  const [generatingPayloads, setGeneratingPayloads] = useState<GeneratorMode | null>(null);
  const [generatedPlan, setGeneratedPlan] = useState<AttackPlanDto | null>(null);
  const [generatedPayloads, setGeneratedPayloads] = useState<PromptPayloadsDto | null>(null);
  const [plannerError, setPlannerError] = useState<string | null>(null);
  const [generatorError, setGeneratorError] = useState<string | null>(null);

  const fingerprintSuggestions = useMemo(
    () => aggregateAttackSuggestions(endpoints, selectedEndpointIds),
    [endpoints, selectedEndpointIds],
  );

  const detectedPlatforms = useMemo(
    () => aggregatePlatformSummary(endpoints, selectedEndpointIds),
    [endpoints, selectedEndpointIds],
  );

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
      generatorMode,
      agentMode,
      maxAgentAttempts,
    });
  }, [profileId, customCategories, disabledTests, activeCategories, generatorMode, agentMode, maxAgentAttempts, onPlanChange]);

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

  function applyGeneratedPlan(plan: AttackPlanDto) {
    const categories = plan.categories.filter((id): id is AttackCategoryId =>
      ALL_ATTACK_CATEGORY_IDS.includes(id as AttackCategoryId),
    );
    if (categories.length === 0) return;
    onPlanUiChange({
      profileId: "custom",
      customCategories: categories,
      disabledTests: plan.disabledTests,
      plannerSummary: plan.summary,
      plannerMode: plan.mode,
    });
  }

  async function handleGeneratePlan(mode: "deterministic" | "local_llm") {
    if (selectedEndpointIds.length === 0) return;
    setGenerating(mode);
    setPlannerError(null);
    try {
      const plan = await generateAttackPlan({
        endpointIds: selectedEndpointIds,
        mode,
      });
      setGeneratedPlan(plan);
      applyGeneratedPlan(plan);
    } catch (err) {
      setPlannerError(toAppError(err).message);
    } finally {
      setGenerating(null);
    }
  }

  async function handleGeneratePayloads(mode: GeneratorMode) {
    if (activeCategories.length === 0) return;
    setGeneratingPayloads(mode);
    setGeneratorError(null);
    try {
      const pack = await generatePromptPayloads({
        profileId,
        categories: activeCategories,
        disabledTests,
        mode,
      });
      setGeneratedPayloads(pack);
      onPlanUiChange({
        generatorMode: mode,
        generatorSummary: pack.summary,
      });
    } catch (err) {
      setGeneratorError(toAppError(err).message);
    } finally {
      setGeneratingPayloads(null);
    }
  }

  function applyFingerprintSuggestions() {
    if (fingerprintSuggestions.categories.length === 0) return;
    onPlanUiChange({
      profileId: "custom",
      customCategories: fingerprintSuggestions.categories,
      disabledTests: [],
    });
  }

  return (
    <div className="wizard-step">
      {detectedPlatforms.length > 0 && (
        <div className="wizard-fingerprint-summary">
          <h4 className="wizard-endpoints__title">Attack Planner</h4>
          <p className="text-muted text-sm">
            Platforms identified before attack execution:{" "}
            {detectedPlatforms
              .map((p) => {
                const flags = [
                  p.memoryEnabled && "memory",
                  p.toolsEnabled && "tools",
                  p.ragEnabled && "RAG",
                ]
                  .filter(Boolean)
                  .join(", ");
                return flags
                  ? `${platformLabel(p.platform)} (${flags})`
                  : platformLabel(p.platform);
              })
              .join(" · ")}
          </p>
          <div className="wizard-fingerprint-summary__actions">
            <Button
              variant="primary"
              type="button"
              disabled={generating !== null || selectedEndpointIds.length === 0}
              onClick={() => void handleGeneratePlan("deterministic")}
            >
              {generating === "deterministic" ? "Planning…" : "Generate (Deterministic)"}
            </Button>
            <Button
              variant="secondary"
              type="button"
              disabled={generating !== null || selectedEndpointIds.length === 0}
              onClick={() => void handleGeneratePlan("local_llm")}
            >
              {generating === "local_llm" ? "Planning…" : "Generate (Local LLM)"}
            </Button>
            {fingerprintSuggestions.categories.length > 0 && (
              <Button variant="ghost" type="button" onClick={applyFingerprintSuggestions}>
                Apply rule suggestions
              </Button>
            )}
          </div>
          {(plannerSummary || generatedPlan?.summary) && (
            <p className="text-sm wizard-planner-summary">
              <strong>Plan:</strong> {plannerSummary ?? generatedPlan?.summary}
              {(plannerMode ?? generatedPlan?.mode) && (
                <Badge variant="muted">{plannerMode ?? generatedPlan?.mode ?? ""}</Badge>
              )}
            </p>
          )}
          {generatedPlan && generatedPlan.rationales.length > 0 && (
            <ul className="wizard-planner-rationales text-sm text-muted">
              {generatedPlan.rationales.slice(0, 6).map((item) => (
                <li key={`${item.category}-${item.source}`}>
                  {getCategory(item.category as AttackCategoryId).label}: {item.reason}
                </li>
              ))}
            </ul>
          )}
          {plannerError && <p className="text-danger text-sm">{plannerError}</p>}
        </div>
      )}

      <div className="wizard-fingerprint-summary">
        <h4 className="wizard-endpoints__title">Payload Generator</h4>
        <p className="text-muted text-sm">
          Build probes from the attack plan for the attack engine ({activeCategories.length}{" "}
          categories).
        </p>
        <div className="wizard-fingerprint-summary__actions">
          <Button
            variant="primary"
            type="button"
            disabled={generatingPayloads !== null || activeCategories.length === 0}
            onClick={() => void handleGeneratePayloads("static_pack")}
          >
            {generatingPayloads === "static_pack" ? "Generating…" : "Static Pack"}
          </Button>
          <Button
            variant="secondary"
            type="button"
            disabled={generatingPayloads !== null || activeCategories.length === 0}
            onClick={() => void handleGeneratePayloads("template_mutation")}
          >
            {generatingPayloads === "template_mutation" ? "Generating…" : "Template Mutation"}
          </Button>
          <Button
            variant="secondary"
            type="button"
            disabled={generatingPayloads !== null || activeCategories.length === 0}
            onClick={() => void handleGeneratePayloads("local_llm")}
          >
            {generatingPayloads === "local_llm" ? "Generating…" : "Local LLM"}
          </Button>
        </div>
        {(generatorSummary || generatedPayloads?.summary) && (
          <p className="text-sm wizard-planner-summary">
            <strong>Payloads:</strong> {generatorSummary ?? generatedPayloads?.summary}
            {(generatorMode || generatedPayloads?.mode) && (
              <Badge variant="muted">{generatorMode ?? generatedPayloads?.mode ?? ""}</Badge>
            )}
          </p>
        )}
        {generatedPayloads && (
          <p className="text-sm text-muted">
            {generatedPayloads.stats.payloadCount} probes across{" "}
            {generatedPayloads.stats.categoryCount} categories
            {generatedPayloads.stats.variantCount > generatedPayloads.stats.payloadCount
              ? ` (${generatedPayloads.stats.variantCount} variants)`
              : ""}
          </p>
        )}
        {generatorError && <p className="text-danger text-sm">{generatorError}</p>}
      </div>

      <div className="wizard-fingerprint-summary">
        <h4 className="wizard-endpoints__title">Agentic Scanner</h4>
        <p className="text-muted text-sm">
          Autonomous loop: fingerprint → plan → attack → judge → retry until a vulnerability is
          found or max attempts are reached.
        </p>
        <label className="wizard-checkbox">
          <input
            type="checkbox"
            checked={agentMode}
            onChange={(event) => onPlanUiChange({ agentMode: event.target.checked })}
          />
          Enable agentic execution
        </label>
        {agentMode && (
          <div className="wizard-agent-options">
            <label className="text-sm">
              Max attempts per category
              <input
                type="number"
                min={1}
                max={20}
                value={maxAgentAttempts}
                onChange={(event) =>
                  onPlanUiChange({
                    maxAgentAttempts: Math.min(20, Math.max(1, Number(event.target.value) || 5)),
                  })
                }
              />
            </label>
          </div>
        )}
      </div>

      {detectedPlatforms.length === 0 && (
        <div className="wizard-fingerprint-summary">
          <h4 className="wizard-endpoints__title">Attack Planner</h4>
          <p className="text-muted text-sm">
            Run discovery with fingerprinted endpoints to generate a dynamic attack plan.
          </p>
        </div>
      )}

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
