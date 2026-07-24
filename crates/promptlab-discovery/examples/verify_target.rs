//! Run: start scripts/discovery-test-target.py, then
//!   cargo run -p promptlab-discovery --example verify_target

use std::time::Duration;

use promptlab_discovery::{DiscoveryConfig, DiscoveryEngine, EndpointKind};

#[tokio::main]
async fn main() {
    let target = std::env::var("DISCOVERY_TARGET").unwrap_or_else(|_| "http://localhost:3000/".into());

    let config = DiscoveryConfig {
        max_depth: 2,
        max_pages: 20,
        worker_count: 4,
        request_timeout: Duration::from_secs(10),
        allow_private_network: true,
        same_origin_only: true,
        probe_static_paths: true,
        ..Default::default()
    };

    println!("=== PromptLab Discovery Verification ===");
    println!("Target: {target}");
    println!("allow_private_network: true\n");

    let engine = DiscoveryEngine::new(config).expect("engine");
    let report = engine.discover(&target).await.expect("discovery");

    println!("Origin: {}", report.origin);
    println!(
        "Stats: pages={} failed={} links={} probes={} duration_ms={}",
        report.stats.pages_fetched,
        report.stats.pages_failed,
        report.stats.links_extracted,
        report.stats.probes_sent,
        report.stats.duration_ms
    );
    println!("Errors ({}): {:?}", report.errors.len(), report.errors);
    println!("Endpoints ({}):", report.endpoints.len());

    for kind in [
        EndpointKind::OpenApi,
        EndpointKind::GraphQl,
        EndpointKind::AiEndpoint,
        EndpointKind::RestApi,
        EndpointKind::Form,
        EndpointKind::JavaScript,
        EndpointKind::Link,
    ] {
        let eps = report.endpoints_by_kind(kind);
        println!("\n[{kind:?}] count={}", eps.len());
        for ep in eps {
            println!(
                "  {} conf={:.2} method={:?} — {}",
                ep.url,
                ep.confidence,
                ep.method,
                ep.evidence
            );
        }
    }

    println!("\n--- JSON ---");
    println!("{}", serde_json::to_string_pretty(&report).expect("json"));
}
