import {
  attackPlanFromDto,
  attackPlanFromExecutionPlaybook,
  attackPlanToDto,
  normalizeAttackPlan,
  type AttackPlanConfig,
  type WizardAttackPlanDto,
} from "./attackPlan";
import {
  createInitialTargetForm,
  targetFormFromDescriptor,
  type TargetFormState,
} from "./targetDescriptor";
import {
  createEmptyVerification,
  createInitialTargetProfile,
  fullProfileUrl,
  profileFromDto,
  type TargetProfileDto,
  type TargetProfileFormState,
  type TargetProviderId,
} from "./targetProfile";
import type { PayloadStrategyDto } from "./payloadStrategy";
import { payloadStrategyToDto } from "./payloadStrategy";
import type { ScanWizardSession } from "./wizardState";
import {
  attackPlanUiFromPlan,
  createInitialAttackPlanUi,
  createInitialSession,
} from "./wizardState";

export const SCAN_CONFIG_FORMAT = "promptlab.scan_config" as const;
export const SCAN_CONFIG_VERSION = 1;
export const SCAN_IMPORT_STORAGE_KEY = "promptlab.scan_import";

export type ScanConfigEndpoint = {
  url: string;
  method: string;
  provider: string;
  framework: string;
  base_url: string;
  path: string;
  headers: Record<string, string>;
  request_template: string;
  prompt_placeholder: string;
  model_field: string;
  streaming_field: string;
  conversation_field: string;
  tool_field: string;
  attachment_field: string;
  verification_strategy: string;
  default_capabilities: TargetProfileFormState["defaultCapabilities"];
};

export type ScanConfigAttack = {
  profile: string;
  categories: string[];
  disabled_tests: string[];
  agent_mode: boolean;
  max_agent_attempts?: number;
  payload_strategy?: PayloadStrategyDto;
  reflection_enabled?: boolean;
  adaptive_planning?: boolean;
  /** Full wizard attack plan — required for Attack Plan / Attack steps on import. */
  plan?: WizardAttackPlanDto;
};

export type ScanConfigExport = {
  format: typeof SCAN_CONFIG_FORMAT;
  version: typeof SCAN_CONFIG_VERSION;
  exported_at: string;
  source_scan_id?: string;
  endpoint: ScanConfigEndpoint;
  auth: Record<string, unknown>;
  attack: ScanConfigAttack;
};

export type ScanConfigParseResult =
  | { ok: true; config: ScanConfigExport }
  | { ok: false; error: string };

export type ScanConfigWizardPatch = {
  targetProfile: TargetProfileFormState;
  targetForm: TargetFormState;
  attackPlan: AttackPlanConfig | null;
  attackPlanUi: ReturnType<typeof createInitialAttackPlanUi>;
  currentStep: 1 | 2;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function asBool(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function headersFromProfile(profile: TargetProfileFormState | TargetProfileDto): Record<string, string> {
  if ("headers" in profile && profile.headers && typeof profile.headers === "object") {
    return { ...(profile.headers as Record<string, string>) };
  }
  try {
    const parsed = JSON.parse((profile as TargetProfileFormState).headersJson) as Record<string, string>;
    return parsed ?? {};
  } catch {
    return {};
  }
}

function looksLikeAttackPlan(value: unknown): value is Record<string, unknown> {
  const record = asRecord(value);
  return Boolean(
    record &&
      typeof record.profileId === "string" &&
      Array.isArray(record.categories) &&
      Array.isArray(record.attackGraph),
  );
}

/** Prefer full wizard snapshot plan; fall back to reconstructing from execution playbook. */
export function resolveAttackPlanFromPlaybook(
  playbook: unknown,
  endpoint = "target",
): AttackPlanConfig | null {
  const root = asRecord(playbook);
  if (!root) return null;

  const wizard = asRecord(root.wizard_snapshot) ?? asRecord(root.wizard);
  const rawPlan = wizard?.attackPlan ?? wizard?.attack_plan;
  if (looksLikeAttackPlan(rawPlan)) {
    const record = rawPlan as Record<string, unknown>;
    // Wizard DB stores AttackPlanConfig — keep summary/metrics/profileModes intact.
    if (typeof record.summary === "string" && record.payloadStrategy) {
      const plan = rawPlan as unknown as AttackPlanConfig;
      return normalizeAttackPlan({
        ...plan,
        customCategories: plan.customCategories?.length ? plan.customCategories : plan.categories,
        disabledGraphNodes: plan.disabledGraphNodes ?? [],
        recommendedPayloadStrategy: plan.recommendedPayloadStrategy ?? plan.payloadStrategy,
        profileModes: plan.profileModes ?? [],
        suggestedCategories: plan.suggestedCategories ?? plan.categories,
        rationales: plan.rationales ?? [],
      });
    }
    try {
      return attackPlanFromDto(rawPlan as unknown as WizardAttackPlanDto);
    } catch {
      // Fall through to execution playbook rebuild.
    }
  }

  return attackPlanFromExecutionPlaybook(playbook, endpoint);
}

function attackSectionFromPlan(plan: AttackPlanConfig): ScanConfigAttack {
  return {
    profile: plan.profileId,
    categories: [...plan.categories],
    disabled_tests: [...plan.disabledTests],
    agent_mode: plan.executionStrategy === "agentic",
    max_agent_attempts: plan.maxAttempts,
    payload_strategy: payloadStrategyToDto(plan.payloadStrategy),
    reflection_enabled: plan.reflectionEnabled,
    adaptive_planning: plan.adaptivePlanning,
    plan: attackPlanToDto(plan),
  };
}

export function buildScanConfigExport(input: {
  scanId?: string;
  profile: TargetProfileFormState | TargetProfileDto;
  descriptor?: unknown;
  playbook?: unknown;
}): ScanConfigExport {
  const profileForm =
    "headersJson" in input.profile
      ? (input.profile as TargetProfileFormState)
      : profileFromDto(input.profile as TargetProfileDto);
  const headers = headersFromProfile(input.profile);
  const url = fullProfileUrl(profileForm);
  const descriptor = asRecord(input.descriptor) ?? {};
  const auth =
    asRecord(descriptor.auth) ??
    ({ kind: "none", engine: "none", method: "none" } satisfies Record<string, unknown>);

  const playbook = asRecord(input.playbook) ?? {};
  const resolvedPlan = resolveAttackPlanFromPlaybook(input.playbook, url);

  let attack: ScanConfigAttack;
  if (resolvedPlan) {
    attack = attackSectionFromPlan(resolvedPlan);
  } else {
    attack = {
      profile: asString(playbook.profile, "standard"),
      categories: asStringArray(playbook.categories),
      disabled_tests: asStringArray(playbook.disabled_tests),
      agent_mode: asBool(playbook.agent_mode, false),
    };
    if (typeof playbook.max_agent_attempts === "number") {
      attack.max_agent_attempts = playbook.max_agent_attempts;
    }
    if (playbook.payload_strategy && typeof playbook.payload_strategy === "object") {
      attack.payload_strategy = playbook.payload_strategy as PayloadStrategyDto;
    }
    if (typeof playbook.reflection_enabled === "boolean") {
      attack.reflection_enabled = playbook.reflection_enabled;
    }
    if (typeof playbook.adaptive_planning === "boolean") {
      attack.adaptive_planning = playbook.adaptive_planning;
    }
  }

  return {
    format: SCAN_CONFIG_FORMAT,
    version: SCAN_CONFIG_VERSION,
    exported_at: new Date().toISOString(),
    source_scan_id: input.scanId,
    endpoint: {
      url,
      method: profileForm.method || "POST",
      provider: profileForm.provider,
      framework: profileForm.framework,
      base_url: profileForm.baseUrl,
      path: profileForm.path,
      headers,
      request_template: profileForm.requestTemplate,
      prompt_placeholder: profileForm.promptPlaceholder,
      model_field: profileForm.modelField,
      streaming_field: profileForm.streamingField,
      conversation_field: profileForm.conversationField,
      tool_field: profileForm.toolField,
      attachment_field: profileForm.attachmentField,
      verification_strategy: profileForm.verificationStrategy,
      default_capabilities: profileForm.defaultCapabilities,
    },
    auth,
    attack,
  };
}

export function parseScanConfigExport(raw: string): ScanConfigParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: "Invalid JSON." };
  }

  const root = asRecord(parsed);
  if (!root) {
    return { ok: false, error: "Scan config must be a JSON object." };
  }
  if (root.format !== SCAN_CONFIG_FORMAT) {
    return { ok: false, error: `Unsupported format (expected ${SCAN_CONFIG_FORMAT}).` };
  }
  if (root.version !== SCAN_CONFIG_VERSION) {
    return { ok: false, error: `Unsupported scan config version (expected ${SCAN_CONFIG_VERSION}).` };
  }

  const endpoint = asRecord(root.endpoint);
  if (!endpoint) {
    return { ok: false, error: "Missing endpoint section." };
  }
  const url =
    asString(endpoint.url) ||
    fullProfileUrl({
      ...createInitialTargetProfile(),
      baseUrl: asString(endpoint.base_url),
      path: asString(endpoint.path, "/"),
    });
  if (!url.trim()) {
    return { ok: false, error: "Endpoint URL is required." };
  }

  const attack = asRecord(root.attack);
  if (!attack) {
    return { ok: false, error: "Missing attack section." };
  }

  const planRaw = attack.plan;
  let plan: WizardAttackPlanDto | undefined;
  if (looksLikeAttackPlan(planRaw)) {
    plan = planRaw as unknown as WizardAttackPlanDto;
  }

  const categories =
    asStringArray(attack.categories).length > 0
      ? asStringArray(attack.categories)
      : plan
        ? [...plan.categories]
        : [];
  if (categories.length === 0) {
    return { ok: false, error: "Attack categories must not be empty." };
  }

  const auth = asRecord(root.auth) ?? { kind: "none", engine: "none", method: "none" };

  const config: ScanConfigExport = {
    format: SCAN_CONFIG_FORMAT,
    version: SCAN_CONFIG_VERSION,
    exported_at: asString(root.exported_at, new Date().toISOString()),
    source_scan_id: typeof root.source_scan_id === "string" ? root.source_scan_id : undefined,
    endpoint: {
      url,
      method: asString(endpoint.method, "POST"),
      provider: asString(endpoint.provider, "openai_compatible"),
      framework: asString(endpoint.framework, "openai_compatible"),
      base_url: asString(endpoint.base_url),
      path: asString(endpoint.path, "/"),
      headers:
        endpoint.headers && typeof endpoint.headers === "object" && !Array.isArray(endpoint.headers)
          ? (endpoint.headers as Record<string, string>)
          : {},
      request_template: asString(endpoint.request_template, "{}"),
      prompt_placeholder: asString(endpoint.prompt_placeholder, "{{PROMPT}}"),
      model_field: asString(endpoint.model_field),
      streaming_field: asString(endpoint.streaming_field),
      conversation_field: asString(endpoint.conversation_field),
      tool_field: asString(endpoint.tool_field),
      attachment_field: asString(endpoint.attachment_field),
      verification_strategy: asString(endpoint.verification_strategy, "openai_chat_completion"),
      default_capabilities:
        (endpoint.default_capabilities as TargetProfileFormState["defaultCapabilities"]) ??
        createInitialTargetProfile().defaultCapabilities,
    },
    auth,
    attack: {
      profile: asString(attack.profile, plan?.profileId ?? "standard"),
      categories,
      disabled_tests:
        asStringArray(attack.disabled_tests).length > 0
          ? asStringArray(attack.disabled_tests)
          : plan
            ? [...plan.disabledTests]
            : [],
      agent_mode: asBool(attack.agent_mode, plan?.executionStrategy === "agentic"),
      max_agent_attempts:
        typeof attack.max_agent_attempts === "number"
          ? attack.max_agent_attempts
          : plan?.maxAttempts,
      payload_strategy:
        attack.payload_strategy && typeof attack.payload_strategy === "object"
          ? (attack.payload_strategy as PayloadStrategyDto)
          : plan?.payloadStrategy,
      reflection_enabled:
        typeof attack.reflection_enabled === "boolean"
          ? attack.reflection_enabled
          : plan?.reflectionEnabled,
      adaptive_planning:
        typeof attack.adaptive_planning === "boolean"
          ? attack.adaptive_planning
          : plan?.adaptivePlanning,
      plan,
    },
  };

  if (!config.endpoint.base_url) {
    try {
      const parsedUrl = new URL(url);
      config.endpoint.base_url = `${parsedUrl.protocol}//${parsedUrl.host}`;
      config.endpoint.path = `${parsedUrl.pathname}${parsedUrl.search}` || "/";
    } catch {
      return { ok: false, error: "Endpoint URL is invalid." };
    }
  }

  return { ok: true, config };
}

function attackPlanFromScanConfig(config: ScanConfigExport, endpointUrl: string): AttackPlanConfig | null {
  if (config.attack.plan && looksLikeAttackPlan(config.attack.plan)) {
    const dto = config.attack.plan;
    const plan = attackPlanFromDto(dto);
    // Keep exported preview fields so Attack / Attack Plan steps match the source scan.
    return normalizeAttackPlan({
      ...plan,
      summary: dto.summary || plan.summary,
      riskScore: dto.riskScore ?? plan.riskScore,
      riskLevel: dto.riskLevel || plan.riskLevel,
      confidence: dto.confidence ?? plan.confidence,
      estimatedRequests: dto.estimatedRequests ?? plan.estimatedRequests,
      estimatedRuntimeSeconds: dto.estimatedRuntimeSeconds ?? plan.estimatedRuntimeSeconds,
      estimatedTokens: dto.estimatedTokens ?? plan.estimatedTokens,
      coverageScore: dto.coverageScore ?? plan.coverageScore,
      riskCoverage: dto.riskCoverage ?? plan.riskCoverage,
      totalTestcases: dto.totalTestcases ?? plan.totalTestcases,
      plannerSource: dto.plannerSource === "ai_runtime" ? "ai_runtime" : plan.plannerSource,
    });
  }

  return attackPlanFromExecutionPlaybook(
    {
      profile: config.attack.profile,
      categories: config.attack.categories,
      disabled_tests: config.attack.disabled_tests,
      agent_mode: config.attack.agent_mode,
      max_agent_attempts: config.attack.max_agent_attempts,
      payload_strategy: config.attack.payload_strategy,
      reflection_enabled: config.attack.reflection_enabled,
      adaptive_planning: config.attack.adaptive_planning,
    },
    endpointUrl,
  );
}

export function scanConfigToWizardPatch(
  config: ScanConfigExport,
  options?: { projectId?: string },
): ScanConfigWizardPatch {
  const targetProfile: TargetProfileFormState = {
    ...createInitialTargetProfile(),
    provider: config.endpoint.provider as TargetProviderId,
    framework: config.endpoint.framework,
    method: config.endpoint.method,
    baseUrl: config.endpoint.base_url,
    path: config.endpoint.path,
    headersJson: JSON.stringify(config.endpoint.headers ?? {}, null, 2),
    requestTemplate: config.endpoint.request_template,
    promptPlaceholder: config.endpoint.prompt_placeholder,
    modelField: config.endpoint.model_field,
    streamingField: config.endpoint.streaming_field,
    conversationField: config.endpoint.conversation_field,
    toolField: config.endpoint.tool_field,
    attachmentField: config.endpoint.attachment_field,
    defaultCapabilities: config.endpoint.default_capabilities,
    verificationStrategy: config.endpoint.verification_strategy,
    verification: createEmptyVerification(),
  };

  const descriptor = {
    url: config.endpoint.url || fullProfileUrl(targetProfile),
    auth: config.auth,
  };
  const targetForm = targetFormFromDescriptor(descriptor, fullProfileUrl(targetProfile));
  const endpointUrl = fullProfileUrl(targetProfile);
  const attackPlan = attackPlanFromScanConfig(config, endpointUrl);
  const normalized = attackPlan ? normalizeAttackPlan(attackPlan) : null;

  return {
    targetProfile,
    targetForm: targetForm.url
      ? targetForm
      : { ...createInitialTargetForm(), url: endpointUrl },
    attackPlan: normalized,
    attackPlanUi: normalized ? attackPlanUiFromPlan(normalized) : createInitialAttackPlanUi(),
    currentStep: options?.projectId?.trim() ? 2 : 1,
  };
}

export function applyScanConfigToSession(
  session: ScanWizardSession,
  config: ScanConfigExport,
): ScanWizardSession {
  const patch = scanConfigToWizardPatch(config, { projectId: session.selectedProjectId });
  return {
    ...session,
    targetProfile: patch.targetProfile,
    targetForm: patch.targetForm,
    attackPlan: patch.attackPlan,
    attackPlanUi: patch.attackPlanUi,
    attackPlanSource: patch.attackPlan ? "imported" : null,
    // Always land on step 1 so the import harness can auto-walk 1→2→3→4.
    currentStep: 1,
    importAutoAdvance: true,
    savedTargetId: null,
    savedTargetFingerprint: null,
    submittedScanId: null,
    draftScanId: null,
    verificationLog: [],
  };
}

/** Sticky import + last hydrated session for Strict Mode remount safety. */
let stickyImport: ScanConfigExport | null = null;
let lastHydratedImport: { projectId: string; session: ScanWizardSession } | null = null;

export function stashScanConfigImport(config: ScanConfigExport): void {
  stickyImport = config;
  lastHydratedImport = null;
  if (typeof window === "undefined") return;
  window.sessionStorage.setItem(SCAN_IMPORT_STORAGE_KEY, JSON.stringify(config));
}

export function peekScanConfigImport(): ScanConfigExport | null {
  if (stickyImport) return stickyImport;
  if (typeof window === "undefined") return null;
  const raw = window.sessionStorage.getItem(SCAN_IMPORT_STORAGE_KEY);
  if (!raw) return null;
  const parsed = parseScanConfigExport(raw);
  if (parsed.ok) {
    stickyImport = parsed.config;
    return parsed.config;
  }
  return null;
}

export function clearScanConfigImport(): void {
  stickyImport = null;
  lastHydratedImport = null;
  if (typeof window === "undefined") return;
  window.sessionStorage.removeItem(SCAN_IMPORT_STORAGE_KEY);
}

export function consumeScanConfigImport(): ScanConfigExport | null {
  const config = peekScanConfigImport();
  stickyImport = null;
  if (typeof window !== "undefined") {
    window.sessionStorage.removeItem(SCAN_IMPORT_STORAGE_KEY);
  }
  return config;
}

export function downloadScanConfigJson(config: ScanConfigExport, filename: string): void {
  const blob = new Blob([JSON.stringify(config, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

/** Build wizard session from stashed import. Pass `consume: true` on the lasting apply. */
export function createSessionFromScanConfigImport(
  projectId = "",
  options?: { consume?: boolean },
): ScanWizardSession {
  const base = createInitialSession(projectId);

  if (!options?.consume) {
    const imported = peekScanConfigImport();
    if (imported) return applyScanConfigToSession(base, imported);
    if (lastHydratedImport && lastHydratedImport.projectId === projectId) {
      return lastHydratedImport.session;
    }
    return base;
  }

  const imported = peekScanConfigImport();
  if (!imported) {
    if (lastHydratedImport && lastHydratedImport.projectId === projectId) {
      return lastHydratedImport.session;
    }
    return base;
  }

  const next = applyScanConfigToSession(base, imported);
  stickyImport = null;
  if (typeof window !== "undefined") {
    window.sessionStorage.removeItem(SCAN_IMPORT_STORAGE_KEY);
  }
  lastHydratedImport = { projectId, session: next };
  return next;
}
