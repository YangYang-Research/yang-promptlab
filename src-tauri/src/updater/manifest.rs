use serde::Deserialize;
use url::Url;

use super::UpdateError;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pub_date: String,
    #[serde(default)]
    pub mandatory: bool,
    pub platforms: std::collections::BTreeMap<String, UpdatePlatformAsset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlatformAsset {
    /// Absolute HTTPS URL or path relative to the manifest URL.
    #[serde(alias = "path")]
    pub url: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default, deserialize_with = "deserialize_size")]
    pub size: Option<u64>,
}

fn deserialize_size<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<u64>::deserialize(deserializer)?;
    Ok(value.filter(|size| *size > 0))
}

pub fn parse_manifest(raw: &str) -> Result<UpdateManifest, UpdateError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(UpdateError::InvalidManifest("empty document".into()));
    }
    if trimmed.len() > 256 * 1024 {
        return Err(UpdateError::InvalidManifest("document exceeds 256 KiB".into()));
    }
    let manifest: UpdateManifest = serde_json::from_str(trimmed)
        .map_err(|err| UpdateError::InvalidManifest(err.to_string()))?;
    if manifest.version.trim().is_empty() {
        return Err(UpdateError::InvalidManifest("missing version".into()));
    }
    if manifest.platforms.is_empty() {
        return Err(UpdateError::InvalidManifest("no platforms".into()));
    }
    Ok(manifest)
}

pub fn current_platform_key() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

pub fn is_newer_version(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(remote), Some(current)) => remote > current,
        _ => false,
    }
}

fn parse_semver(input: &str) -> Option<(u64, u64, u64)> {
    let cleaned = input.trim().trim_start_matches('v');
    let core = cleaned.split(['-', '+']).next().unwrap_or(cleaned);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

pub fn resolve_asset(
    manifest: &UpdateManifest,
    manifest_url: &str,
    platform: &str,
) -> Result<ResolvedAsset, UpdateError> {
    let asset = manifest.platforms.get(platform).ok_or_else(|| {
        UpdateError::InvalidManifest(format!("no asset for platform {platform}"))
    })?;
    let url = resolve_url(manifest_url, asset.url.trim())?;
    let filename = sanitize_filename(if asset.filename.trim().is_empty() {
        filename_from_url(&url)
    } else {
        asset.filename.trim()
    })?;
    Ok(ResolvedAsset {
        url,
        filename,
        sha256: asset.sha256.trim().to_ascii_lowercase(),
        size: asset.size,
    })
}

#[derive(Debug, Clone)]
pub struct ResolvedAsset {
    pub url: String,
    pub filename: String,
    pub sha256: String,
    pub size: Option<u64>,
}

pub fn resolve_url(manifest_url: &str, asset: &str) -> Result<String, UpdateError> {
    if asset.is_empty() {
        return Err(UpdateError::InvalidManifest("empty asset path".into()));
    }
    let resolved = if looks_like_absolute_url(asset) {
        Url::parse(asset).map_err(|err| UpdateError::InvalidManifest(err.to_string()))?
    } else {
        let base = Url::parse(manifest_url)
            .map_err(|err| UpdateError::InvalidManifest(format!("invalid manifest URL: {err}")))?;
        base.join(asset)
            .map_err(|err| UpdateError::InvalidManifest(format!("invalid asset path: {err}")))?
    };
    validate_https_url(&resolved)?;
    Ok(resolved.to_string())
}

fn looks_like_absolute_url(value: &str) -> bool {
    value.contains("://")
}

pub fn validate_https_url(url: &Url) -> Result<(), UpdateError> {
    match url.scheme() {
        "https" => {}
        "http" if is_loopback_host(url.host_str()) => {}
        other => {
            return Err(UpdateError::UnsafeUrl(format!(
                "scheme '{other}' is not allowed"
            )));
        }
    }
    if url.host_str().is_none() {
        return Err(UpdateError::UnsafeUrl("missing host".into()));
    }
    Ok(())
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1" | "localhost" | "::1"))
}

pub fn sanitize_filename(name: &str) -> Result<String, UpdateError> {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim();
    if base.is_empty() || base == "." || base == ".." {
        return Err(UpdateError::InvalidManifest("invalid installer filename".into()));
    }
    if base.chars().any(|ch| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' ))
    }) {
        return Err(UpdateError::InvalidManifest(format!(
            "unsafe installer filename '{base}'"
        )));
    }
    Ok(base.to_string())
}

fn filename_from_url(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "schemaVersion": 1,
      "version": "0.2.0",
      "notes": "bugfix",
      "platforms": {
        "darwin-aarch64": {
          "path": "PromptLab-0.2.0-darwin-aarch64.dmg",
          "sha256": "abc",
          "size": 12
        },
        "windows-x86_64": {
          "url": "https://example.com/PromptLab-0.2.0-windows-x64-setup.exe",
          "sha256": "def"
        }
      }
    }"#;

    #[test]
    fn parses_manifest_and_resolves_relative_path() {
        let manifest = parse_manifest(SAMPLE).expect("parse");
        assert_eq!(manifest.version, "0.2.0");
        let asset = resolve_asset(
            &manifest,
            "https://example.com/manifests/version.json",
            "darwin-aarch64",
        )
        .expect("resolve");
        assert_eq!(
            asset.url,
            "https://example.com/manifests/PromptLab-0.2.0-darwin-aarch64.dmg"
        );
        assert_eq!(asset.filename, "PromptLab-0.2.0-darwin-aarch64.dmg");
        assert_eq!(asset.sha256, "abc");
        assert_eq!(asset.size, Some(12));
    }

    #[test]
    fn version_compare_treats_patch_bumps_as_newer() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("v0.1.1", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.2.0"));
        assert!(!is_newer_version("not-a-version", "0.1.0"));
    }

    #[test]
    fn rejects_non_https_asset_urls() {
        let err = resolve_url("https://example.com/version.json", "file:///tmp/evil").unwrap_err();
        match err {
            UpdateError::UnsafeUrl(_) => {}
            other => panic!("expected UnsafeUrl, got {other}"),
        }
    }

    #[test]
    fn sanitize_filename_strips_path_and_rejects_traversal() {
        assert_eq!(
            sanitize_filename("foo/PromptLab-1.dmg").unwrap(),
            "PromptLab-1.dmg"
        );
        assert_eq!(sanitize_filename("../evil.dmg").unwrap(), "evil.dmg");
        assert!(sanitize_filename("..").is_err());
        assert!(sanitize_filename("a b.exe").is_err());
    }

    #[test]
    fn rejects_empty_or_huge_manifest() {
        assert!(parse_manifest("").is_err());
        let huge = format!("{{\"version\":\"1.0.0\",\"platforms\":{{}},\"pad\":\"{}\"}}", "x".repeat(300_000));
        assert!(parse_manifest(&huge).is_err());
    }

    #[test]
    fn committed_version_json_parses() {
        let manifest = parse_manifest(include_str!("../../../manifests/version.json")).expect("parse");
        assert!(!manifest.version.is_empty());
        for key in ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"] {
            assert!(manifest.platforms.contains_key(key), "missing {key}");
        }
    }
}
