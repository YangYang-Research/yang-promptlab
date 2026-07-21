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

type WeightDraft = {
  judge: string;
  classifier: string;
  attacker: string;
  defaultLlm: string;
};

const ROLE_FIELDS: Array<{ key: keyof WeightDraft; label: string; hint: string }> = [
  { key: "judge", label: "Judge", hint: "Primary vulnerability verdict" },
  { key: "classifier", label: "Classifier", hint: "Category / severity labeling" },
  { key: "attacker", label: "Attacker", hint: "Adversarial confirmation" },
  { key: "defaultLlm", label: "Other / no role", hint: "Fallback when role is missing" },
];

function toDraft(weights: Omit<JudgeRoleWeightsDto, "updatedAt">): WeightDraft {
  return {
    judge: String(weights.judge),
    classifier: String(weights.classifier),
    attacker: String(weights.attacker),
    defaultLlm: String(weights.defaultLlm),
  };
}

function parseWeight(raw: string, label: string): number {
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0.01 || value > 2) {
    throw new Error(`${label} weight must be between 0.01 and 2.0`);
  }
  return value;
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

  const handleSave = async () => {
    setSaving(true);
    try {
      const request = {
        judge: parseWeight(draft.judge, "Judge"),
        classifier: parseWeight(draft.classifier, "Classifier"),
        attacker: parseWeight(draft.attacker, "Attacker"),
        defaultLlm: parseWeight(draft.defaultLlm, "Other / no role"),
      };
      const weights = await setJudgeRoleWeights(request);
      const next = toDraft(weights);
      setDraft(next);
      setSaved(next);
      toast.notify("Judge role weights saved", "success");
    } catch (err) {
      toast.notify(toAppError(err).message, "error");
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    setDraft(toDraft(DEFAULT_JUDGE_ROLE_WEIGHTS));
  };

  return (
    <Card className="detail-section runtime-page__weights">
      <div className="detail-section__header">
        <div>
          <h2 className="detail-section__title">Judge role weights</h2>
          <p className="detail-section__hint">
            Relative influence of each Yazg role when aggregating scan confidence. Stored in the
            local database and applied on the next judge run.
          </p>
        </div>
        <div className="detail-section__header-actions">
          <Button
            variant="secondary"
            size="sm"
            disabled={disabled || loading || saving}
            onClick={handleReset}
          >
            Reset defaults
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={disabled || loading || saving || !dirty}
            onClick={() => void handleSave()}
          >
            {saving ? "Saving…" : "Save weights"}
          </Button>
        </div>
      </div>

      <div className="runtime-weights-grid" role="group" aria-label="Judge role weights">
        {ROLE_FIELDS.map((field) => (
          <label key={field.key} className="runtime-weights-field">
            <span className="runtime-weights-field__label">{field.label}</span>
            <span className="runtime-weights-field__hint">{field.hint}</span>
            <input
              className="runtime-weights-field__input mono"
              type="number"
              min={0.01}
              max={2}
              step={0.05}
              disabled={disabled || loading || saving}
              value={draft[field.key]}
              onChange={(event) =>
                setDraft((prev) => ({ ...prev, [field.key]: event.target.value }))
              }
            />
          </label>
        ))}
      </div>
    </Card>
  );
}
