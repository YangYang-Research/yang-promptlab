//! Real Playwright/Chromium SPA capture demo.
//!
//! Run:
//!   python3 scripts/spa-test-target.py
//!   cargo run -p aisec-discovery --example browser_capture
//!
//! Requires the bundled Playwright runner dependencies:
//!   (cd crates/aisec-discovery/playwright && npm install && npx playwright install chromium)

use aisec_discovery::{BrowserConfig, DiscoveryConfig, DiscoveryEngine, EndpointKind};

#[tokio::main]
async fn main() {
    let target =
        std::env::var("BROWSER_TARGET").unwrap_or_else(|_| "http://localhost:3100/".into());

    // HTTP discovery config (probes off so the demo isolates browser value).
    let config = DiscoveryConfig {
        max_depth: 1,
        max_pages: 10,
        worker_count: 4,
        allow_private_network: true,
        probe_static_paths: false,
        ..Default::default()
    };
    let engine = DiscoveryEngine::new(config).expect("engine");

    let browser = BrowserConfig {
        headless: true,
        settle_ms: 1500,
        ..Default::default()
    };

    println!("=== AISec Browser (Playwright/Chromium) Capture ===");
    println!("Target: {target}\n");

    let report = engine
        .discover_with_browser(&target, browser)
        .await
        .expect("discover_with_browser");

    println!("Endpoints ({} total):", report.endpoints.len());
    for kind in [
        EndpointKind::AiEndpoint,
        EndpointKind::RestApi,
        EndpointKind::GraphQl,
        EndpointKind::JavaScript,
        EndpointKind::Form,
        EndpointKind::Link,
    ] {
        let eps = report.endpoints_by_kind(kind);
        if eps.is_empty() {
            continue;
        }
        println!("\n[{kind:?}] count={}", eps.len());
        for ep in eps {
            println!(
                "  {} {} conf={:.2} — {}",
                ep.method.as_deref().unwrap_or("-"),
                ep.url,
                ep.confidence,
                ep.evidence
            );
        }
    }

    println!("\nErrors ({}): {:?}", report.errors.len(), report.errors);
}
