//! Centralized OCSF structured logging via an in-process event bus.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// OCSF-inspired severity levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OcsfSeverity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl OcsfSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Informational => "informational",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Log category — maps to dedicated log files under `logs/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogCategory {
    Application,
    System,
    Runtime,
    Models,
    Authentication,
    Harness,
    Planner,
    PayloadGenerator,
    AttackEngine,
    Judge,
    Workspace,
    Projects,
    Plugins,
    Settings,
    UserInterface,
    Scan,
    Agent,
}

impl LogCategory {
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Application => "Application Activity",
            Self::System => "System Activity",
            Self::Runtime => "Runtime Activity",
            Self::Models => "Model Activity",
            Self::Authentication => "Authentication",
            Self::Harness => "Harness Execution",
            Self::Planner => "Attack Planning",
            Self::PayloadGenerator => "Payload Generation",
            Self::AttackEngine => "Attack Execution",
            Self::Judge => "Judge Evaluation",
            Self::Workspace => "Workspace Activity",
            Self::Projects => "Project Activity",
            Self::Plugins => "Plugin Activity",
            Self::Settings => "Settings Activity",
            Self::UserInterface => "User Interface",
            Self::Scan => "Scan Activity",
            Self::Agent => "Agent Activity",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Application => "app.log",
            Self::System => "system.log",
            Self::Runtime => "runtime.log",
            Self::Models => "models.log",
            Self::Authentication => "auth.log",
            Self::Harness => "harness.log",
            Self::Planner => "planner.log",
            Self::PayloadGenerator => "payload.log",
            Self::AttackEngine => "attack.log",
            Self::Judge => "judge.log",
            Self::Workspace => "workspace.log",
            Self::Projects => "projects.log",
            Self::Plugins => "plugins.log",
            Self::Settings => "settings.log",
            Self::UserInterface => "ui.log",
            Self::Scan => "scan.log",
            Self::Agent => "agents.log",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::System => "system",
            Self::Runtime => "runtime",
            Self::Models => "models",
            Self::Authentication => "authentication",
            Self::Harness => "harness",
            Self::Planner => "planner",
            Self::PayloadGenerator => "payload_generator",
            Self::AttackEngine => "attack_engine",
            Self::Judge => "judge",
            Self::Workspace => "workspace",
            Self::Projects => "projects",
            Self::Plugins => "plugins",
            Self::Settings => "settings",
            Self::UserInterface => "user_interface",
            Self::Scan => "scan",
            Self::Agent => "agent",
        }
    }
}

/// OCSF-shaped JSON log event (JSON Lines).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcsfEvent {
    pub timestamp: String,
    pub severity: String,
    pub category: String,
    #[serde(rename = "classUid")]
    pub class_uid: u32,
    #[serde(rename = "className")]
    pub class_name: String,
    #[serde(rename = "activityId")]
    pub activity_id: u32,
    #[serde(rename = "activityName")]
    pub activity_name: String,
    pub module: String,
    pub component: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub attributes: Map<String, Value>,
}

impl OcsfEvent {
    pub fn new(
        category: LogCategory,
        severity: OcsfSeverity,
        activity_name: impl Into<String>,
        module: impl Into<String>,
        component: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let activity = activity_name.into();
        let ts = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "now".into());
        Self {
            timestamp: ts,
            severity: severity.as_str().into(),
            category: category.as_str().into(),
            class_uid: category_class_uid(category),
            class_name: category.class_name().into(),
            activity_id: 1,
            activity_name: activity,
            module: module.into(),
            component: component.into(),
            workspace_id: None,
            project_id: None,
            scan_id: None,
            message: mask_secrets(&message.into()),
            attributes: Map::new(),
        }
    }

    pub fn with_context(mut self, workspace_id: Option<String>, project_id: Option<String>, scan_id: Option<String>) -> Self {
        self.workspace_id = workspace_id;
        self.project_id = project_id;
        self.scan_id = scan_id;
        self
    }

    pub fn attr(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.attributes.insert(key.into(), mask_value(v));
        }
        self
    }
}

fn category_class_uid(category: LogCategory) -> u32 {
    match category {
        LogCategory::Application => 1001,
        LogCategory::System => 1002,
        LogCategory::Runtime => 2001,
        LogCategory::Models => 2002,
        LogCategory::Authentication => 3001,
        LogCategory::Harness => 4001,
        LogCategory::Planner => 4002,
        LogCategory::PayloadGenerator => 4003,
        LogCategory::AttackEngine => 4004,
        LogCategory::Judge => 4005,
        LogCategory::Workspace => 5001,
        LogCategory::Projects => 5002,
        LogCategory::Plugins => 6001,
        LogCategory::Settings => 7001,
        LogCategory::UserInterface => 8001,
        LogCategory::Scan => 9001,
        LogCategory::Agent => 4010,
    }
}

static SECRET_PATTERNS: &[&str] = &[
    "api_key",
    "apikey",
    "password",
    "bearer",
    "authorization",
    "cookie",
    "session_token",
    "private_key",
    "secret",
    "token",
];

pub fn mask_secrets(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    for needle in SECRET_PATTERNS {
        if lower.contains(needle) {
            return "[REDACTED]".into();
        }
    }
    if lower.starts_with("basic ") || lower.starts_with("bearer ") {
        return "[REDACTED]".into();
    }
    input.to_string()
}

fn mask_value(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(mask_secrets(&s)),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                let key_lower = k.to_ascii_lowercase();
                if SECRET_PATTERNS.iter().any(|p| key_lower.contains(p)) {
                    out.insert(k, Value::String("[REDACTED]".into()));
                } else {
                    out.insert(k, mask_value(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(mask_value).collect()),
        other => other,
    }
}

/// In-process event bus — components publish; logger thread writes files.
#[derive(Clone)]
pub struct EventBus {
    tx: mpsc::Sender<OcsfEvent>,
}

impl EventBus {
    pub fn publish(&self, event: OcsfEvent) {
        let _ = self.tx.send(event);
    }

    pub fn info(
        &self,
        category: LogCategory,
        activity: &str,
        module: &str,
        component: &str,
        message: impl Into<String>,
    ) {
        self.publish(OcsfEvent::new(
            category,
            OcsfSeverity::Informational,
            activity,
            module,
            component,
            message,
        ));
    }

    pub fn error(
        &self,
        category: LogCategory,
        activity: &str,
        module: &str,
        component: &str,
        message: impl Into<String>,
    ) {
        self.publish(OcsfEvent::new(
            category,
            OcsfSeverity::High,
            activity,
            module,
            component,
            message,
        ));
    }
}

pub struct EventLogGuard {
    _writer: JoinHandle<()>,
}

/// Ring buffer of recent events for Troubleshooting UI.
#[derive(Default)]
pub struct EventRing {
    inner: Mutex<Vec<OcsfEvent>>,
    capacity: usize,
}

impl EventRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn push(&self, event: &OcsfEvent) {
        let mut guard = self.inner.lock().expect("event ring lock");
        if guard.len() >= self.capacity {
            guard.remove(0);
        }
        guard.push(event.clone());
    }

    pub fn recent(&self, limit: usize) -> Vec<OcsfEvent> {
        let guard = self.inner.lock().expect("event ring lock");
        let start = guard.len().saturating_sub(limit);
        guard[start..].to_vec()
    }
}

static GLOBAL_BUS: OnceLock<Arc<EventBus>> = OnceLock::new();
static GLOBAL_RING: OnceLock<Arc<EventRing>> = OnceLock::new();

pub fn global_event_bus() -> Option<Arc<EventBus>> {
    GLOBAL_BUS.get().cloned()
}

pub fn global_event_ring() -> Option<Arc<EventRing>> {
    GLOBAL_RING.get().cloned()
}

/// Start the centralized logger — the only component that writes log files.
pub fn spawn_event_logger(logs_dir: PathBuf) -> (EventBus, Arc<EventRing>, EventLogGuard) {
    let (tx, rx) = mpsc::channel();
    let bus = EventBus { tx };
    let ring = Arc::new(EventRing::new(2000));
    let ring_writer = ring.clone();
    let writer = thread::spawn(move || {
        run_log_writer(&logs_dir, rx, ring_writer);
    });
    let bus_arc = Arc::new(bus.clone());
    let _ = GLOBAL_BUS.set(bus_arc);
    let _ = GLOBAL_RING.set(ring.clone());
    (bus, ring, EventLogGuard { _writer: writer })
}

fn run_log_writer(logs_dir: &Path, rx: mpsc::Receiver<OcsfEvent>, ring: Arc<EventRing>) {
    let mut handles: HashMap<String, BufWriter<File>> = HashMap::new();

    for event in rx {
        ring.push(&event);
        if let Ok(line) = serde_json::to_string(&event) {
            let line = format!("{line}\n");
            write_category_log(logs_dir, &mut handles, event.category_file(), &line);
            if event.category_file() != "app.log" {
                write_category_log(logs_dir, &mut handles, "app.log", &line);
            }
        }
    }
}

trait CategoryFile {
    fn category_file(&self) -> &'static str;
}

impl CategoryFile for OcsfEvent {
    fn category_file(&self) -> &'static str {
        match self.category.as_str() {
            "system" => "system.log",
            "runtime" => "runtime.log",
            "models" => "models.log",
            "authentication" => "auth.log",
            "harness" => "harness.log",
            "planner" => "planner.log",
            "payload_generator" => "payload.log",
            "attack_engine" => "attack.log",
            "judge" => "judge.log",
            "workspace" => "workspace.log",
            "projects" => "projects.log",
            "plugins" => "plugins.log",
            "settings" => "settings.log",
            "user_interface" => "ui.log",
            "scan" => "scan.log",
            "agent" => "agents.log",
            _ => "app.log",
        }
    }
}

fn write_category_log(
    logs_dir: &Path,
    handles: &mut HashMap<String, BufWriter<File>>,
    file_name: &str,
    line: &str,
) {
    let path = logs_dir.join(file_name);
    let writer = handles.entry(file_name.to_string()).or_insert_with(|| {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|_| File::create(&path).expect("log file"));
        BufWriter::new(file)
    });
    let _ = writer.write_all(line.as_bytes());
    let _ = writer.flush();
}

pub fn read_log_tail(path: &Path, max_bytes: usize) -> std::io::Result<String> {
    let data = std::fs::read(path)?;
    if data.len() <= max_bytes {
        return Ok(String::from_utf8_lossy(&data).into_owned());
    }
    Ok(String::from_utf8_lossy(&data[data.len() - max_bytes..]).into_owned())
}

pub fn list_log_files(logs_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(logs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("log") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

pub fn publish_crash(
    bus: &EventBus,
    module: &str,
    message: &str,
    stack_trace: &str,
    env_snapshot: Value,
) {
    let event = OcsfEvent::new(
        LogCategory::System,
        OcsfSeverity::Critical,
        "Unhandled Exception",
        module,
        "panic_hook",
        message,
    )
    .attr("stackTrace", stack_trace)
    .attr("environment", env_snapshot);
    bus.publish(event.clone());
    bus.publish(OcsfEvent::new(
        LogCategory::Application,
        OcsfSeverity::Critical,
        "Unhandled Exception",
        module,
        "panic_hook",
        message,
    )
    .attr("stackTrace", stack_trace));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_secrets_in_message() {
        assert_eq!(mask_secrets("Bearer abc123"), "[REDACTED]");
        assert_eq!(mask_secrets("hello world"), "hello world");
    }

    #[test]
    fn event_logger_writes_json_line() {
        let dir = tempfile::tempdir().unwrap();
        let (bus, _ring, _guard) = spawn_event_logger(dir.path().to_path_buf());
        bus.info(
            LogCategory::Application,
            "Application Started",
            "test",
            "test",
            "boot",
        );
        thread::sleep(std::time::Duration::from_millis(50));
        let app_log = dir.path().join("app.log");
        assert!(app_log.exists());
    }
}
