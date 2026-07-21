//! Per-scan Attack Console log files under `{logs_dir}/scan-console/{scan_id}.log`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use url::Url;

use crate::events::ScanProgressEvent;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

const SCAN_CONSOLE_DIR: &str = "scan-console";

pub fn scan_console_log_path(logs_dir: &Path, scan_id: &str) -> Option<PathBuf> {
    let safe_id = sanitize_scan_id(scan_id)?;
    Some(logs_dir.join(SCAN_CONSOLE_DIR).join(format!("{safe_id}.log")))
}

pub fn clear_log(logs_dir: &Path, scan_id: &str) -> std::io::Result<()> {
    let Some(path) = scan_console_log_path(logs_dir, scan_id) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid scan id",
        ));
    };

    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn append_line(logs_dir: &Path, event: &ScanProgressEvent) -> std::io::Result<()> {
    let Some(path) = scan_console_log_path(logs_dir, &event.scan_id) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid scan id",
        ));
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let line = format_console_line(event);
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

pub fn read_from_offset(
    logs_dir: &Path,
    scan_id: &str,
    offset: usize,
) -> std::io::Result<(String, usize, usize)> {
    let Some(path) = scan_console_log_path(logs_dir, scan_id) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid scan id",
        ));
    };

    if !path.exists() {
        return Ok((String::new(), offset, offset));
    }

    let data = fs::read(&path)?;
    let total = data.len();
    if offset >= total {
        return Ok((String::new(), total, total));
    }

    let chunk = String::from_utf8_lossy(&data[offset..]).into_owned();
    Ok((chunk, total, total))
}

fn sanitize_scan_id(scan_id: &str) -> Option<&str> {
    if scan_id.is_empty()
        || scan_id.contains('/')
        || scan_id.contains('\\')
        || scan_id.contains("..")
    {
        return None;
    }
    if !scan_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    Some(scan_id)
}

fn format_console_line(event: &ScanProgressEvent) -> String {
    let stamp = OffsetDateTime::parse(&event.timestamp, &Rfc3339)
        .ok()
        .and_then(|t| {
            t.format(&time::format_description::parse("[hour]:[minute]:[second]").ok()?)
                .ok()
        })
        .unwrap_or_else(|| "--:--:--".into());

    let mut parts = vec![format!("[{stamp}]"), event.message.clone()];

    if let Some(endpoint) = &event.endpoint {
        let path = Url::parse(endpoint)
            .map(|url| url.path().to_string())
            .unwrap_or_else(|_| endpoint.clone());
        parts.push(format!("@ {path}"));
    }

    if let Some(payload) = &event.payload {
        parts.push(format!("\n    payload: {payload}"));
    }

    if let Some(response) = &event.response {
        parts.push(format!("\n    response: {response}"));
    }

    if let Some(code) = event.status_code {
        let latency = event
            .latency
            .map(|ms| format!(" {ms}ms"))
            .unwrap_or_default();
        parts.push(format!("→ {code}{latency}"));
    }

    if let Some(finding_id) = &event.finding_id {
        let short = finding_id.chars().take(8).collect::<String>();
        parts.push(format!("[finding {short}]"));
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{ScanProgressEvent, ScanProgressLevel};

    #[test]
    fn append_and_read_scan_console_log() {
        let dir = tempfile::tempdir().unwrap();
        let event = ScanProgressEvent::new("scan-abc-123", ScanProgressLevel::Info, "Probe sent")
            .endpoint("https://api.example.com/v1/chat")
            .status_code(200)
            .latency(42);

        append_line(dir.path(), &event).unwrap();
        let (content, offset, total) = read_from_offset(dir.path(), "scan-abc-123", 0).unwrap();
        assert!(content.contains("Probe sent"));
        assert!(content.contains("@ /v1/chat"));
        assert!(content.contains("→ 200 42ms"));
        assert_eq!(offset, total);
        assert!(total > 0);

        let (more, next, _) = read_from_offset(dir.path(), "scan-abc-123", offset).unwrap();
        assert!(more.is_empty());
        assert_eq!(next, offset);
    }

    #[test]
    fn clear_log_removes_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let event = ScanProgressEvent::new("scan-abc-123", ScanProgressLevel::Info, "Probe sent");
        append_line(dir.path(), &event).unwrap();
        assert!(scan_console_log_path(dir.path(), "scan-abc-123").unwrap().exists());

        clear_log(dir.path(), "scan-abc-123").unwrap();
        assert!(!scan_console_log_path(dir.path(), "scan-abc-123").unwrap().exists());

        let (content, _, total) = read_from_offset(dir.path(), "scan-abc-123", 0).unwrap();
        assert!(content.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn rejects_path_traversal_scan_id() {
        assert!(scan_console_log_path(Path::new("/tmp"), "../evil").is_none());
    }
}
