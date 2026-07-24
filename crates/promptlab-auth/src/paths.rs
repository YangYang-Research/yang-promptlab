use std::path::{Path, PathBuf};

/// Platform-specific PromptLab data root (without bundle id suffix).
pub fn default_data_root() -> PathBuf {
    if let Some(base) = dirs_for_platform() {
        return base.join("PromptLab");
    }
    PathBuf::from(".promptlab")
}

/// Auth session vault directory for encrypted Playwright artifacts.
///
/// - Windows: `%LOCALAPPDATA%/PromptLab/AuthSessions`
/// - macOS: `~/Library/Application Support/PromptLab/AuthSessions`
/// - Linux: `~/.local/share/promptlab/AuthSessions`
pub fn auth_sessions_dir(data_root: impl AsRef<Path>) -> PathBuf {
    data_root.as_ref().join("AuthSessions")
}

fn dirs_for_platform() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local/share"))
            })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_sessions_dir_suffix() {
        let dir = auth_sessions_dir("/tmp/promptlab-data");
        assert!(dir.ends_with("AuthSessions"));
    }
}
