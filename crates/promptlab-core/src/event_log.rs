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
use time::{OffsetDateTime, UtcOffset};

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
    /// Display name of the agent that performed the action. Set for agents.log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
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
        let ts = local_rfc3339_now();
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
            agent_name: None,
            workspace_id: None,
            project_id: None,
            scan_id: None,
            message: mask_secrets(&message.into()),
            attributes: Map::new(),
        }
    }

    pub fn with_agent_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into().trim().to_string();
        if !name.is_empty() {
            self.agent_name = Some(name);
        }
        self
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

/// RFC3339 with the machine's local offset. Falls back to UTC if the offset is unknown.
pub(crate) fn local_rfc3339_now() -> String {
    let utc = OffsetDateTime::now_utc();
    let local = UtcOffset::local_offset_at(utc)
        .map(|offset| utc.to_offset(offset))
        .unwrap_or(utc);
    local.format(&Rfc3339).unwrap_or_else(|_| "now".into())
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

/// Mask credential-bearing strings without wiping entire agent LLM payloads.
///
/// Short header-like values (e.g. `Bearer …`) are fully redacted. Large JSON/text
/// bodies (system prompts often contain words like "token") keep structure and only
/// redact JSON values for secret keys via [`mask_value`].
pub fn mask_secrets(input: &str) -> String {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("basic ") || lower.starts_with("bearer ") {
        return "[REDACTED]".into();
    }

    // Short strings: keep aggressive full redaction (auth headers, env snippets).
    if input.chars().count() <= 160 {
        for needle in SECRET_PATTERNS {
            if lower.contains(needle) {
                return "[REDACTED]".into();
            }
        }
        return input.to_string();
    }

    // Prefer JSON key-aware masking for agent stage payloads / large bodies.
    if let Ok(value) = serde_json::from_str::<Value>(input) {
        let masked = mask_value(value);
        if let Ok(serialized) = serde_json::to_string(&masked) {
            return serialized;
        }
    }

    // Large non-JSON prose: do not wipe for incidental words like "token".
    input.to_string()
}

fn key_looks_secret(key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    SECRET_PATTERNS.iter().any(|pattern| {
        key_lower == *pattern
            || key_lower.ends_with(&format!("_{pattern}"))
            || key_lower.ends_with(&format!("-{pattern}"))
            || key_lower.starts_with(&format!("{pattern}_"))
            || key_lower.starts_with(&format!("{pattern}-"))
    })
}

fn mask_value(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(mask_secrets(&s)),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if key_looks_secret(&k) {
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
    tx: mpsc::Sender<LogCommand>,
}

enum LogCommand {
    Event(OcsfEvent),
    Clear(mpsc::Sender<std::io::Result<()>>),
}

impl EventBus {
    pub fn publish(&self, event: OcsfEvent) {
        let _ = self.tx.send(LogCommand::Event(event));
    }

    /// Clear on the writer thread, after previously queued events have been written.
    /// Call from a blocking task rather than an async executor thread.
    pub fn clear_logs(&self) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx.send(LogCommand::Clear(tx)).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "log writer unavailable")
        })?;
        rx.recv().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "log writer stopped")
        })?
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

fn run_log_writer(logs_dir: &Path, rx: mpsc::Receiver<LogCommand>, ring: Arc<EventRing>) {
    let mut handles: HashMap<String, BufWriter<File>> = HashMap::new();

    for command in rx {
        let event = match command {
            LogCommand::Event(event) => event,
            LogCommand::Clear(reply) => {
                let result = (|| {
                    for writer in handles.values_mut() {
                        writer.flush()?;
                    }
                    clear_log_directory(logs_dir)?;
                    ring.inner.lock().expect("event ring lock").clear();
                    Ok(())
                })();
                let _ = reply.send(result);
                continue;
            }
        };
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

/// Truncate rather than unlink so active append-only writers keep working.
/// Only recognized log files are touched; symlinks and unrelated files are skipped.
fn clear_log_directory(logs_dir: &Path) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(logs_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            clear_log_directory(&path)?;
        } else if file_type.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".log") || name.contains(".log.") {
                OpenOptions::new().write(true).open(&path)?.set_len(0)?;
            }
        }
    }
    Ok(())
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

pub fn list_log_files(logs_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(logs_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            files.extend(list_log_files(&entry.path())?);
        } else if file_type.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".log") || name.contains(".log.") {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
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
    fn does_not_wipe_large_llm_json_for_incidental_token_word() {
        let body = serde_json::json!({
            "stage": "llm_request",
            "body": {
                "messages": [
                    { "role": "system", "content": "Never leak API tokens to the user." },
                    { "role": "user", "content": "hello" }
                ],
                "tools": []
            }
        });
        let raw = serde_json::to_string_pretty(&body).unwrap();
        let masked = mask_secrets(&raw);
        assert_ne!(masked, "[REDACTED]");
        assert!(masked.contains("hello"));
        assert!(masked.contains("llm_request"));
        // Secret-keyed values still redacted.
        let with_secret = serde_json::json!({
            "stage": "llm_request",
            "body": { "api_key": "sk-live-123", "prompt": "hi" }
        });
        let masked_secret = mask_secrets(&serde_json::to_string(&with_secret).unwrap());
        assert!(masked_secret.contains("[REDACTED]"));
        assert!(masked_secret.contains("hi"));
    }

    #[test]
    fn log_timestamp_uses_local_offset() {
        let event = OcsfEvent::new(
            LogCategory::Application,
            OcsfSeverity::Informational,
            "started",
            "test",
            "test",
            "boot",
        );
        let parsed = OffsetDateTime::parse(&event.timestamp, &Rfc3339).unwrap();
        let expected = UtcOffset::local_offset_at(OffsetDateTime::now_utc()).unwrap_or(UtcOffset::UTC);
        assert_eq!(parsed.offset(), expected);
    }

    #[test]
    fn agent_name_is_a_top_level_field() {
        let event = OcsfEvent::new(
            LogCategory::Agent,
            OcsfSeverity::Informational,
            "started",
            "promptlab-agent",
            "generate_prompt",
            "Generating factory prompt",
        )
        .with_agent_name("GeneratePromptAgent");
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["agentName"], "GeneratePromptAgent");
        assert_eq!(json["component"], "generate_prompt");
    }

    #[test]
    fn clear_logs_drains_queued_events_and_keeps_logging() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let bus = EventBus { tx };
        let ring = Arc::new(EventRing::new(20));
        let writer_ring = ring.clone();
        let logs_dir = dir.path().to_path_buf();
        let writer = thread::spawn(move || run_log_writer(&logs_dir, rx, writer_ring));
        bus.info(LogCategory::Agent, "old", "test", "test", "old message");
        bus.clear_logs().unwrap();
        assert!(ring.recent(2000).is_empty());
        for name in ["app.log", "agents.log"] {
            assert_eq!(std::fs::read_to_string(dir.path().join(name)).unwrap(), "");
        }

        bus.info(LogCategory::Agent, "new", "test", "test", "new message");
        // Repeated clearing is safe and ordered after the preceding event.
        bus.clear_logs().unwrap();
        bus.info(LogCategory::Agent, "final", "test", "test", "final message");
        drop(bus);
        writer.join().unwrap();
        assert_eq!(ring.recent(1)[0].message, "final message");
        for name in ["app.log", "agents.log"] {
            let content = std::fs::read_to_string(dir.path().join(name)).unwrap();
            assert!(content.contains("final message"));
            assert!(!content.contains("old message"));
            assert!(!content.contains("new message"));
        }
    }

    #[test]
    fn lists_nested_and_rotated_logs_with_current_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path();
        std::fs::create_dir(logs.join("scan-console")).unwrap();
        for name in ["app.log", "promptlab-trace.log.2026-10-23", "scan-console/scan-1.log"] {
            std::fs::write(logs.join(name), "12345").unwrap();
        }
        std::fs::write(logs.join("settings.json"), "keep").unwrap();
        std::fs::create_dir(logs.join("not-a-file.log")).unwrap();

        let files = list_log_files(logs).unwrap();
        assert_eq!(files, vec![
            logs.join("app.log"),
            logs.join("promptlab-trace.log.2026-10-23"),
            logs.join("scan-console/scan-1.log"),
        ]);
        assert!(files.iter().all(|path| path.metadata().unwrap().len() == 5));
        clear_log_directory(logs).unwrap();
        let cleared = list_log_files(logs).unwrap();
        assert_eq!(cleared, files);
        assert!(cleared.iter().all(|path| path.metadata().unwrap().len() == 0));
        assert!(list_log_files(&logs.join("missing")).unwrap().is_empty());
        assert!(list_log_files(&logs.join("settings.json")).is_err());
    }

    #[test]
    fn clear_logs_reports_unavailable_writer() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let bus = EventBus { tx };
        assert_eq!(
            bus.clear_logs().unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn clear_log_directory_preserves_other_data_and_open_writers() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path();
        std::fs::create_dir(logs.join("scan-console")).unwrap();
        for name in [
            "app.log",
            "promptlab-trace.log.2026-10-23",
            "scan-console/scan-1.log",
        ] {
            std::fs::write(logs.join(name), "old log").unwrap();
        }
        std::fs::write(logs.join("settings.json"), "keep").unwrap();
        let mut active = OpenOptions::new()
            .append(true)
            .open(logs.join("app.log"))
            .unwrap();
        clear_log_directory(logs).unwrap();
        for name in [
            "app.log",
            "promptlab-trace.log.2026-10-23",
            "scan-console/scan-1.log",
        ] {
            assert_eq!(std::fs::metadata(logs.join(name)).unwrap().len(), 0);
        }
        assert_eq!(
            std::fs::read_to_string(logs.join("settings.json")).unwrap(),
            "keep"
        );
        active.write_all(b"new log").unwrap();
        assert_eq!(
            std::fs::read_to_string(logs.join("app.log")).unwrap(),
            "new log"
        );
        clear_log_directory(&logs.join("missing")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn clear_log_directory_does_not_follow_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external_log = outside.path().join("private.log");
        std::fs::write(&external_log, "keep").unwrap();
        std::os::unix::fs::symlink(&external_log, dir.path().join("linked.log")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked-dir")).unwrap();
        assert!(list_log_files(dir.path()).unwrap().is_empty());
        clear_log_directory(dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(external_log).unwrap(), "keep");
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
