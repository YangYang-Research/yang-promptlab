export function huggingFaceRepoUrl(downloadUrl: string | null): string | null {
  if (!downloadUrl) return null;
  const match = downloadUrl.match(/huggingface\.co\/([^/]+\/[^/]+)/i);
  return match ? `https://huggingface.co/${match[1]}` : null;
}

export function huggingFaceModelIcon(name: string): string {
  const initial = name.trim().charAt(0).toUpperCase() || "M";
  return initial;
}
