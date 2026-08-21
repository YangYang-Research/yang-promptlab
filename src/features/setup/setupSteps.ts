export type SetupStepId =
  | "runtime-mode"
  | "add-model"
  | "load-model"
  | "first-project-scan";

export type SetupStep = {
  id: SetupStepId;
  title: string;
  description: string;
  to: string;
  done: boolean;
  /** True when a prior step is still incomplete — not clickable yet. */
  locked: boolean;
};

export type SetupProgressInput = {
  mode: "not_configured" | "third_party" | "local";
  initialized: boolean;
  localModelCount: number;
  configuredThirdPartyCount: number;
  selectedModelId: string | null;
  modelLoaded: boolean;
  projectCount: number;
  scanCount: number;
};

const STEP_META: Array<{
  id: SetupStepId;
  title: string;
  description: string;
  to: string;
}> = [
  {
    id: "runtime-mode",
    title: "Choose AI Runtime Mode",
    description: "Pick Local or Third-party API in AI Runtime.",
    to: "/runtime",
  },
  {
    id: "add-model",
    title: "Add Model",
    description: "Install a local model or configure an API model.",
    to: "/models",
  },
  {
    id: "load-model",
    title: "Load model into AI Runtime",
    description: "Select and activate the model for inference.",
    to: "/runtime",
  },
  {
    id: "first-project-scan",
    title: "Create a Project and first scan",
    description: "Create a project, then start your first scan.",
    to: "/scans/new",
  },
];

export function deriveSetupSteps(input: SetupProgressInput): SetupStep[] {
  const rawDone = [
    input.initialized && input.mode !== "not_configured",
    input.localModelCount > 0 || input.configuredThirdPartyCount > 0,
    input.mode === "local"
      ? Boolean(input.selectedModelId) && input.modelLoaded
      : input.mode === "third_party"
        ? Boolean(input.selectedModelId) && input.configuredThirdPartyCount > 0
        : false,
    input.projectCount > 0 && input.scanCount > 0,
  ];

  // Enforce strict order: a step can only be done if every prior step is done.
  const sequentialDone: boolean[] = [];
  for (let i = 0; i < rawDone.length; i += 1) {
    sequentialDone[i] = Boolean(rawDone[i] && (i === 0 || sequentialDone[i - 1]));
  }

  return STEP_META.map((meta, index) => {
    const done = sequentialDone[index] ?? false;
    const locked = index > 0 && !(sequentialDone[index - 1] ?? false);
    return { ...meta, done, locked };
  });
}

export function setupProgress(steps: SetupStep[]): {
  doneCount: number;
  total: number;
  allDone: boolean;
} {
  const doneCount = steps.filter((step) => step.done).length;
  return {
    doneCount,
    total: steps.length,
    allDone: doneCount === steps.length && steps.length > 0,
  };
}
