export type TargetAuthKind = "none" | "basic" | "api_key";

export type TargetDescriptorInput = {
  url: string;
  authKind: TargetAuthKind;
  username?: string;
  password?: string;
  headerName?: string;
  headerValue?: string;
};

export function deriveTargetName(url: string): string {
  const trimmed = url.trim();
  try {
    return new URL(trimmed).hostname || trimmed;
  } catch {
    return trimmed;
  }
}

export function validateTargetStep(input: TargetDescriptorInput): string | null {
  const url = input.url.trim();
  if (!url) {
    return "Target URL is required.";
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "URL must use http or https.";
    }
  } catch {
    return "Enter a valid URL (e.g. https://api.example.com).";
  }

  if (input.authKind === "basic") {
    if (!input.username?.trim()) return "Username is required for basic auth.";
    if (!input.password) return "Password is required for basic auth.";
  }

  if (input.authKind === "api_key") {
    if (!input.headerName?.trim()) return "Header name is required for API key auth.";
    if (!input.headerValue?.trim()) return "API key value is required.";
  }

  return null;
}

/** Build `descriptor_json` persisted by `target_create` (url + auth). */
export function buildTargetDescriptor(input: TargetDescriptorInput): Record<string, unknown> {
  const url = input.url.trim();
  const descriptor: Record<string, unknown> = { url };

  switch (input.authKind) {
    case "basic":
      descriptor.auth = {
        kind: "basic",
        username: input.username!.trim(),
        password: input.password,
      };
      break;
    case "api_key":
      descriptor.auth = {
        kind: "api_key",
        header: input.headerName!.trim(),
        value: input.headerValue!.trim(),
      };
      break;
    default:
      descriptor.auth = { kind: "none" };
  }

  return descriptor;
}
