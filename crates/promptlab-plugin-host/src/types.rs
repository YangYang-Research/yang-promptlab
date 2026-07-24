use serde::{Deserialize, Serialize};

/// Supported plugin implementation language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginLanguage {
    Python,
    JavaScript,
}

impl PluginLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
        }
    }

    pub fn default_interpreter(self) -> &'static str {
        match self {
            Self::Python => "python3",
            Self::JavaScript => "node",
        }
    }
}

/// Plugin category aligned with AISec engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    Discovery,
    Attack,
    Judge,
    Report,
}

impl PluginType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discovery => "discovery",
            Self::Attack => "attack",
            Self::Judge => "judge",
            Self::Report => "report",
        }
    }

    pub fn default_hook(self) -> &'static str {
        match self {
            Self::Discovery => "discover",
            Self::Attack => "execute_attack",
            Self::Judge => "evaluate",
            Self::Report => "render_report",
        }
    }
}

/// Plugin lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    Discovered,
    Installed,
    Enabled,
    Loaded,
    Active,
    Disabled,
    Error,
}

impl PluginState {
    pub fn can_transition_to(self, next: PluginState) -> bool {
        use PluginState::*;
        matches!(
            (self, next),
            (Discovered, Installed)
                | (Installed, Enabled)
                | (Installed, Disabled)
                | (Enabled, Loaded)
                | (Loaded, Active)
                | (Active, Loaded)
                | (Loaded, Disabled)
                | (Disabled, Enabled)
                | (Disabled, Installed)
                | (Installed, Discovered)
                | (_, Error)
                | (Error, Disabled)
        )
    }
}

/// Capability permission flags.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginPermissions {
    #[serde(default)]
    pub probe_mutate: bool,
    #[serde(default)]
    pub finding_emit: bool,
    #[serde(default)]
    pub http_request: bool,
    #[serde(default)]
    pub filesystem_read: Vec<String>,
    #[serde(default)]
    pub filesystem_write: bool,
    #[serde(default)]
    pub log: bool,
}

impl PluginPermissions {
    pub fn minimal() -> Self {
        Self {
            log: true,
            ..Default::default()
        }
    }

    pub fn allows(&self, capability: HostCapability) -> bool {
        match capability {
            HostCapability::ProbeMutate => self.probe_mutate,
            HostCapability::FindingEmit => self.finding_emit,
            HostCapability::HttpRequest => self.http_request,
            HostCapability::FilesystemRead => !self.filesystem_read.is_empty(),
            HostCapability::FilesystemWrite => self.filesystem_write,
            HostCapability::Log => self.log,
        }
    }
}

/// Host capabilities plugins may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCapability {
    ProbeMutate,
    FindingEmit,
    HttpRequest,
    FilesystemRead,
    FilesystemWrite,
    Log,
}

/// Installed plugin record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub plugin_type: PluginType,
    pub language: PluginLanguage,
    pub install_path: std::path::PathBuf,
    pub state: PluginState,
    pub permissions: PluginPermissions,
    pub hooks: PluginHooks,
    pub enabled: bool,
}

/// Hook name mapping per plugin type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginHooks {
    #[serde(default)]
    pub discover: Option<String>,
    #[serde(default)]
    pub execute_attack: Option<String>,
    #[serde(default)]
    pub evaluate: Option<String>,
    #[serde(default)]
    pub render_report: Option<String>,
}

impl PluginHooks {
    pub fn hook_for(&self, plugin_type: PluginType) -> Option<&str> {
        match plugin_type {
            PluginType::Discovery => self.discover.as_deref(),
            PluginType::Attack => self.execute_attack.as_deref(),
            PluginType::Judge => self.evaluate.as_deref(),
            PluginType::Report => self.render_report.as_deref(),
        }
    }
}

/// Result of invoking a plugin hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInvokeResult {
    pub plugin_id: String,
    pub hook: String,
    pub result: serde_json::Value,
    pub host_calls: Vec<HostCallRecord>,
    pub duration_ms: u64,
}

/// Record of a host API call made by the plugin during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCallRecord {
    pub method: String,
    pub allowed: bool,
    pub params: serde_json::Value,
}

/// Sandbox execution limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub allow_network_env: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            max_output_bytes: 4 * 1024 * 1024,
            allow_network_env: false,
        }
    }
}
