use std::path::PathBuf;
use std::process::Command;

use aisec_plugin_host::{PluginManager, PluginType};

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/samples")
        .canonicalize()
        .expect("plugins/samples directory")
}

fn runtime_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn discovers_all_sample_plugins() {
    let mut mgr = PluginManager::new(samples_dir()).unwrap();
    let ids = mgr.discover().unwrap();
    assert_eq!(ids.len(), 4, "expected four sample plugins");
    assert_eq!(mgr.by_type(PluginType::Discovery).len(), 1);
    assert_eq!(mgr.by_type(PluginType::Attack).len(), 1);
    assert_eq!(mgr.by_type(PluginType::Judge).len(), 1);
    assert_eq!(mgr.by_type(PluginType::Report).len(), 1);
}

#[tokio::test]
async fn invokes_discovery_sample() {
    if !runtime_available("python3") {
        eprintln!("skipping: python3 not available");
        return;
    }

    let mut mgr = PluginManager::new(samples_dir()).unwrap();
    mgr.discover().unwrap();
    let id = "com.aisec.sample.discovery-openapi";
    mgr.enable(id).unwrap();

    let result = mgr
        .invoke(
            id,
            serde_json::json!({"target_url": "https://api.example.com"}),
        )
        .await
        .unwrap();

    assert_eq!(result.hook, "discover");
    assert!(result.result.get("count").and_then(|v| v.as_u64()).unwrap() > 0);
    assert!(result.host_calls.iter().any(|c| c.method == "log" && c.allowed));
}

#[tokio::test]
async fn invokes_attack_sample() {
    if !runtime_available("node") {
        eprintln!("skipping: node not available");
        return;
    }

    let mut mgr = PluginManager::new(samples_dir()).unwrap();
    mgr.discover().unwrap();
    let id = "com.aisec.sample.attack-delimiter";
    mgr.enable(id).unwrap();

    let result = mgr
        .invoke(
            id,
            serde_json::json!({"payload": "test injection"}),
        )
        .await
        .unwrap();

    assert_eq!(result.result.get("technique").and_then(|v| v.as_str()), Some("delimiter_injection"));
    assert!(result.host_calls.iter().any(|c| c.method == "probe_mutate" && c.allowed));
}

#[tokio::test]
async fn invokes_judge_sample() {
    if !runtime_available("python3") {
        eprintln!("skipping: python3 not available");
        return;
    }

    let mut mgr = PluginManager::new(samples_dir()).unwrap();
    mgr.discover().unwrap();
    let id = "com.aisec.sample.judge-keyword";
    mgr.enable(id).unwrap();

    let result = mgr
        .invoke(
            id,
            serde_json::json!({"response_text": "Here is the password: hunter2"}),
        )
        .await
        .unwrap();

    assert_eq!(result.result.get("vulnerable").and_then(|v| v.as_bool()), Some(true));
}

#[tokio::test]
async fn invokes_report_sample() {
    if !runtime_available("node") {
        eprintln!("skipping: node not available");
        return;
    }

    let mut mgr = PluginManager::new(samples_dir()).unwrap();
    mgr.discover().unwrap();
    let id = "com.aisec.sample.report-markdown";
    mgr.enable(id).unwrap();

    let result = mgr
        .invoke(
            id,
            serde_json::json!({
                "project_name": "Demo",
                "findings": [
                    {"title": "Prompt leak", "severity": "high"},
                    {"title": "Weak filter", "severity": "medium"}
                ]
            }),
        )
        .await
        .unwrap();

    assert_eq!(result.result.get("format").and_then(|v| v.as_str()), Some("markdown"));
    assert!(result
        .result
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("Executive Summary"));
}
