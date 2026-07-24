use crate::error::{PluginError, PluginResult};
use crate::types::{HostCapability, PluginPermissions};

/// Validates host API calls against plugin permissions.
pub struct PermissionGuard {
    permissions: PluginPermissions,
}

impl PermissionGuard {
    pub fn new(permissions: PluginPermissions) -> Self {
        Self { permissions }
    }

    pub fn check(&self, method: &str) -> PluginResult<HostCapability> {
        let cap = method_to_capability(method)?;
        if !self.permissions.allows(cap) {
            return Err(PluginError::permission_denied(format!(
                "plugin lacks capability for {method}"
            )));
        }
        Ok(cap)
    }

    pub fn check_path_read(&self, path: &str) -> PluginResult<()> {
        if !self.permissions.allows(HostCapability::FilesystemRead) {
            return Err(PluginError::permission_denied("filesystem_read not granted"));
        }
        let allowed = self.permissions.filesystem_read.iter().any(|pattern| {
            glob_match(pattern, path)
        });
        if !allowed {
            return Err(PluginError::permission_denied(format!(
                "path not in allowlist: {path}"
            )));
        }
        Ok(())
    }
}

fn method_to_capability(method: &str) -> PluginResult<HostCapability> {
    match method {
        "probe_mutate" | "mutate_probe" => Ok(HostCapability::ProbeMutate),
        "emit_finding" => Ok(HostCapability::FindingEmit),
        "http_request" => Ok(HostCapability::HttpRequest),
        "read_resource" | "filesystem_read" => Ok(HostCapability::FilesystemRead),
        "filesystem_write" => Ok(HostCapability::FilesystemWrite),
        "log" => Ok(HostCapability::Log),
        other => Err(PluginError::permission_denied(format!(
            "unknown host method: {other}"
        ))),
    }
}

/// Minimal glob matcher supporting `**` and `*`.
fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "**" || pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix);
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path.starts_with(prefix)
            && !path[prefix.len() + 1..].contains('/');
    }
    pattern == path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_without_capability() {
        let guard = PermissionGuard::new(PluginPermissions::minimal());
        assert!(guard.check("emit_finding").is_err());
        assert!(guard.check("log").is_ok());
    }

    #[test]
    fn path_glob() {
        let perms = PluginPermissions {
            filesystem_read: vec!["$PLUGIN_DIR/**".into()],
            ..Default::default()
        };
        let guard = PermissionGuard::new(perms);
        assert!(guard.check_path_read("/plugins/foo/bar.txt").is_ok());
    }
}
