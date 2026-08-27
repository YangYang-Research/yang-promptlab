import { useCallback, useEffect, useState } from "react";

import { Button, Card } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  DEFAULT_JUDGE_ROLE_WEIGHTS,
  getJudgeRoleWeights,
  setJudgeRoleWeights,
  type JudgeRoleWeightsDto,
} from "@/shared/ipc/runtime";
import { useToast } from "@/shared/notifications";

const WEIGHT_MIN = 0.01;
const WEIGHT_MAX = 2;
const WEIGHT_STEP = 0.05;
const AUTO_SAVE_DELAY_MS = 500;

type WeightKey = "judge" | "classifier" | "attacker" | "defaultLlm";

type WeightDraft = Record<WeightKey, number>;
type PresetProfileId = "balanced" | "judgeBiased" | "consensusBiased";

const ROLE_FIELDS: Array<{ key: WeightKey; label: string; hint: string }> = [
  { key: "judge", label: "JudgeWorker", hint: "Primary vulnerability verdict" },
  { key: "classifier", label: "ClassifierWorker", hint: "Category / severity labeling" },
  { key: "attacker", label: "AttackerWorker", hint: "Adversarial confirmation" },
  { key: "defaultLlm", label: "Other / no role", hint: "Fallback when role is missing" },
];
const PRESET_PROFILES: Array<{
  id: PresetProfileId;
  label: string;
  hint: string;
  values: WeightDraft;
}> = [
  {
    id: "balanced",
    label: "Balanced",
    hint: "Equal priority for JudgeWorker / ClassifierWorker / AttackerWorker",
    values: { judge: 0.75, classifier: 0.75, attacker: 0.75, defaultLlm: 0.65 },
  },
  {
    id: "judgeBiased",
    label: "Judge-biased",
    hint: "Prefer JudgeWorker verdict over other workers",
    values: { judge: 0.95, classifier: 0.75, attacker: 0.65, defaultLlm: 0.6 },
  },
  {
    id: "consensusBiased",
    label: "Consensus-biased",
    hint: "Keep workers closer to reduce role bias",
    values: { judge: 0.85, classifier: 0.8, attacker: 0.75, defaultLlm: 0.65 },
  },
];

function toDraft(weights: Omit<JudgeRoleWeightsDto, "updatedAt">): WeightDraft {
  return {
    judge: weights.judge,
    classifier: weights.classifier,
    attacker: weights.attacker,
    defaultLlm: weights.defaultLlm,
  };
}

function sliderPercent(value: number): number {
  return ((value - WEIGHT_MIN) / (WEIGHT_MAX - WEIGHT_MIN)) * 100;
}

function sameDraft(left: WeightDraft, right: WeightDraft): boolean {
  return ROLE_FIELDS.every((field) => left[field.key] === right[field.key]);
}

function WeightSlider({
  label,
  hint,
  value,
  disabled,
  onChange,
}: {
  label: string;
  hint: string;
  value: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  const fillPct = sliderPercent(value);

  return (
    <div className="settings-judge-weight">
      <div className="settings-judge-weight__header">
        <div>
          <span className="settings-judge-weight__label">{label}</span>
          <span className="settings-judge-weight__hint">{hint}</span>
        </div>
        <span className="settings-judge-weight__value mono">{value.toFixed(2)}</span>
      </div>
      <div className="settings-judge-weight__track-wrap">
        <div className="progress__track" aria-hidden>
          <div className="progress__fill" style={{ width: `${fillPct}%` }} />
        </div>
        <input
          type="range"
          className="settings-judge-weight__input"
          min={WEIGHT_MIN}
          max={WEIGHT_MAX}
          step={WEIGHT_STEP}
          value={value}
          disabled={disabled}
          onChange={(event) => onChange(Number(event.target.value))}
          aria-valuemin={WEIGHT_MIN}
          aria-valuemax={WEIGHT_MAX}
          aria-valuenow={value}
          aria-label={label}
        />
      </div>
    </div>
  );
}

export function JudgeRoleWeightsPanel({ disabled = false }: { disabled?: boolean }) {
  const toast = useToast();
  const [draft, setDraft] = useState<WeightDraft>(() => toDraft(DEFAULT_JUDGE_ROLE_WEIGHTS));
  const [saved, setSaved] = useState<WeightDraft>(() => toDraft(DEFAULT_JUDGE_ROLE_WEIGHTS));
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const weights = await getJudgeRoleWeights();
      const next = toDraft(weights);
      setDraft(next);
      setSaved(next);
    } catch (err) {
      toast.notify(toAppError(err).message, "error");
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const dirty = ROLE_FIELDS.some((field) => draft[field.key] !== saved[field.key]);
  const selectedPreset =
    PRESET_PROFILES.find((preset) => sameDraft(draft, preset.values))?.id ?? null;
  
  useEffect(() => {
    if (loading || disabled || !dirty) return;
    const timeout = window.setTimeout(() => {
      void (async () => {
        setSaving(true);
        try {
          const weights = await setJudgeRoleWeights(draft);
          const next = toDraft(weights);
          setDraft(next);
          setSaved(next);
          toast.notify("Judge worker weights saved", "success");
        } catch (err) {
          toast.notify(toAppError(err).message, "error");
        } finally {
          setSaving(false);
        }
      })();
    }, AUTO_SAVE_DELAY_MS);
    return () => {
      window.clearTimeout(timeout);
    };
  }, [dirty, disabled, draft, loading, toast]);

  const handleReset = () => {
    setDraft(toDraft(DEFAULT_JUDGE_ROLE_WEIGHTS));
  };

  const handleApplyPreset = (presetId: PresetProfileId) => {
    const preset = PRESET_PROFILES.find((entry) => entry.id === presetId);
    if (!preset) return;
    setDraft(preset.values);
  };

  return (
    <Card>
      {loading ? (
        <p className="text-muted text-sm">Loading weights…</p>
      ) : (
        <>
          <div className="settings-judge-presets" role="group" aria-label="Weight presets">
            {PRESET_PROFILES.map((preset) => {
              const active = selectedPreset === preset.id;
              return (
                <span key={preset.id} title={preset.hint}>
                  <Button
                    variant={active ? "primary" : "secondary"}
                    size="sm"
                    disabled={disabled || saving}
                    onClick={() => handleApplyPreset(preset.id)}
                  >
                    {preset.label}
                  </Button>
                </span>
              );
            })}
          </div>
          <div className="settings-judge-weights" role="group" aria-label="Judge worker weights">
            {ROLE_FIELDS.map((field) => (
              <WeightSlider
                key={field.key}
                label={field.label}
                hint={field.hint}
                value={draft[field.key]}
                disabled={disabled || saving}
                onChange={(value) => setDraft((prev) => ({ ...prev, [field.key]: value }))}
              />
            ))}
          </div>
        </>
      )}

      <div className="settings-section__actions">
        <Button
          variant="secondary"
          size="sm"
          disabled={disabled || loading || saving}
          onClick={handleReset}
        >
          Reset defaults
        </Button>
      </div>
    </Card>
  );
}
