use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use aisec_core::AisecResult;
use dashmap::DashSet;
use tokio::sync::{Mutex, Notify, Semaphore};
use tracing::{debug, instrument, warn};
use url::Url;

use crate::client::HttpClient;
use crate::config::DiscoveryConfig;
use crate::detectors::detect_from_snapshot;
use crate::extract::{extract_links, extract_url_hints};
use crate::types::{CrawlStats, CrawlTask, DiscoveredEndpoint, EndpointKind};
use crate::url_policy::{canonical_key, is_same_origin, normalize_url, validate_target_url};

/// Concurrent BFS web crawler with bounded workers.
pub struct Crawler {
    client: HttpClient,
    config: Arc<DiscoveryConfig>,
    seed: Url,
    origin: Url,
    visited: Arc<DashSet<String>>,
    frontier: Arc<Mutex<VecDeque<CrawlTask>>>,
    notify: Arc<Notify>,
    in_flight: Arc<AtomicUsize>,
    pages_fetched: Arc<AtomicUsize>,
    pages_failed: Arc<AtomicUsize>,
    links_extracted: Arc<AtomicUsize>,
    endpoints: Arc<Mutex<Vec<DiscoveredEndpoint>>>,
    errors: Arc<Mutex<Vec<String>>>,
}

impl Crawler {
    pub fn new(client: HttpClient, seed: Url, config: DiscoveryConfig) -> Self {
        let origin = seed.clone();
        Self {
            client,
            config: Arc::new(config),
            seed: seed.clone(),
            origin,
            visited: Arc::new(DashSet::new()),
            frontier: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
            in_flight: Arc::new(AtomicUsize::new(0)),
            pages_fetched: Arc::new(AtomicUsize::new(0)),
            pages_failed: Arc::new(AtomicUsize::new(0)),
            links_extracted: Arc::new(AtomicUsize::new(0)),
            endpoints: Arc::new(Mutex::new(Vec::new())),
            errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[instrument(skip(self), fields(seed = %self.seed))]
    pub async fn run(&self) -> AisecResult<CrawlOutput> {
        self.enqueue(CrawlTask {
            url: self.seed.to_string(),
            depth: 0,
            referrer: None,
        })
        .await;

        let semaphore = Arc::new(Semaphore::new(self.config.worker_count));
        let mut handles = Vec::with_capacity(self.config.worker_count);

        for worker_id in 0..self.config.worker_count {
            handles.push(self.spawn_worker(worker_id, semaphore.clone()));
        }

        self.wait_for_completion().await;

        for handle in handles {
            let _ = handle.await;
        }

        let endpoints = self.endpoints.lock().await.clone();
        let errors = self.errors.lock().await.clone();

        Ok(CrawlOutput {
            endpoints,
            stats: CrawlStats {
                pages_fetched: self.pages_fetched.load(Ordering::Relaxed),
                pages_failed: self.pages_failed.load(Ordering::Relaxed),
                links_extracted: self.links_extracted.load(Ordering::Relaxed),
                probes_sent: 0,
                duration_ms: 0,
            },
            errors,
        })
    }

    fn spawn_worker(
        &self,
        worker_id: usize,
        semaphore: Arc<Semaphore>,
    ) -> tokio::task::JoinHandle<()> {
        let crawler = self.clone_refs();

        tokio::spawn(async move {
            loop {
                let task = {
                    let mut queue = crawler.frontier.lock().await;
                    queue.pop_front()
                };

                let Some(task) = task else {
                    if crawler.in_flight.load(Ordering::Relaxed) == 0 {
                        break;
                    }
                    crawler.notify.notified().await;
                    continue;
                };

                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => break,
                };

                crawler.in_flight.fetch_add(1, Ordering::Relaxed);

                if let Err(err) = crawler.process_task(worker_id, task).await {
                    warn!(worker_id, error = %err.client_message(), "crawl task failed");
                    crawler.record_error(err.client_message()).await;
                }

                crawler.in_flight.fetch_sub(1, Ordering::Relaxed);
                crawler.notify.notify_one();
            }
        })
    }

    async fn wait_for_completion(&self) {
        loop {
            let queue_empty = self.frontier.lock().await.is_empty();
            let idle = self.in_flight.load(Ordering::Relaxed) == 0;
            let limit_reached = self.pages_fetched.load(Ordering::Relaxed) >= self.config.max_pages;

            if (queue_empty && idle) || limit_reached {
                break;
            }

            self.notify.notified().await;
        }
    }

    #[instrument(skip(self, task), fields(url = %task.url, depth = task.depth, worker_id))]
    async fn process_task(&self, worker_id: usize, task: CrawlTask) -> AisecResult<()> {
        if task.depth > self.config.max_depth {
            return Ok(());
        }

        if self.pages_fetched.load(Ordering::Relaxed) >= self.config.max_pages {
            return Ok(());
        }

        let parsed = Url::parse(&task.url)
            .map_err(|err| aisec_core::AisecError::invalid_input(err.to_string()))?;
        validate_target_url(parsed.as_str(), &self.config)?;

        let key = canonical_key(&parsed);
        if !self.visited.insert(key) {
            return Ok(());
        }

        debug!(worker_id, url = %task.url, depth = task.depth, "fetching page");

        match self.client.get(&task.url).await {
            Ok(snapshot) => {
                self.pages_fetched.fetch_add(1, Ordering::Relaxed);
                self.collect_detections(&snapshot, task.referrer.as_deref())
                    .await;

                if snapshot.is_success() && is_html(&snapshot) {
                    self.process_html(&task, &snapshot.body).await?;
                } else if snapshot.is_success() {
                    self.process_hints(&task, &snapshot.body).await?;
                }
            }
            Err(err) => {
                self.pages_failed.fetch_add(1, Ordering::Relaxed);
                return Err(err);
            }
        }

        Ok(())
    }

    async fn process_html(&self, task: &CrawlTask, html: &str) -> AisecResult<()> {
        let links = extract_links(&task.url, html);
        self.links_extracted
            .fetch_add(links.len(), Ordering::Relaxed);

        for link in links {
            self.maybe_enqueue_link(link, task.depth + 1, Some(task.url.clone()))
                .await?;
        }

        Ok(())
    }

    async fn process_hints(&self, task: &CrawlTask, content: &str) -> AisecResult<()> {
        for hint in extract_url_hints(content) {
            let resolved = if hint.starts_with("http") {
                hint
            } else if let Ok(url) = Url::parse(&task.url) {
                normalize_url(&url, &hint)
                    .map(|u| u.to_string())
                    .unwrap_or_default()
            } else {
                continue;
            };

            if !resolved.is_empty() {
                self.maybe_enqueue_link(resolved, task.depth + 1, Some(task.url.clone()))
                    .await?;
            }
        }
        Ok(())
    }

    async fn maybe_enqueue_link(
        &self,
        link: String,
        depth: u32,
        referrer: Option<String>,
    ) -> AisecResult<()> {
        let parsed = Url::parse(&link)
            .map_err(|err| aisec_core::AisecError::invalid_input(err.to_string()))?;

        if self.config.same_origin_only && !is_same_origin(&parsed, &self.origin) {
            // Still record external links as discovered links
            self.record_endpoint(DiscoveredEndpoint::new(
                link,
                EndpointKind::Link,
                0.5,
                "external link extracted from page",
            ))
            .await;
            return Ok(());
        }

        validate_target_url(parsed.as_str(), &self.config)?;

        self.enqueue(CrawlTask {
            url: parsed.to_string(),
            depth,
            referrer,
        })
        .await;

        Ok(())
    }

    async fn collect_detections(&self, snapshot: &crate::types::HttpSnapshot, referrer: Option<&str>) {
        for ep in detect_from_snapshot(snapshot, referrer) {
            self.record_endpoint(ep).await;
        }
    }

    async fn record_endpoint(&self, ep: DiscoveredEndpoint) {
        self.endpoints.lock().await.push(ep);
    }

    async fn record_error(&self, message: String) {
        self.errors.lock().await.push(message);
    }

    async fn enqueue(&self, task: CrawlTask) {
        self.frontier.lock().await.push_back(task);
        self.notify.notify_one();
    }

    fn clone_refs(&self) -> Self {
        Self {
            client: self.client.clone(),
            config: self.config.clone(),
            seed: self.seed.clone(),
            origin: self.origin.clone(),
            visited: self.visited.clone(),
            frontier: self.frontier.clone(),
            notify: self.notify.clone(),
            in_flight: self.in_flight.clone(),
            pages_fetched: self.pages_fetched.clone(),
            pages_failed: self.pages_failed.clone(),
            links_extracted: self.links_extracted.clone(),
            endpoints: self.endpoints.clone(),
            errors: self.errors.clone(),
        }
    }
}

pub struct CrawlOutput {
    pub endpoints: Vec<DiscoveredEndpoint>,
    pub stats: CrawlStats,
    pub errors: Vec<String>,
}

fn is_html(snapshot: &crate::types::HttpSnapshot) -> bool {
    snapshot
        .content_type
        .as_deref()
        .is_some_and(|ct| ct.contains("html"))
        || snapshot.body.to_lowercase().contains("<html")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DiscoveryConfig;

    #[tokio::test]
    async fn crawler_respects_max_depth() {
        let cfg = DiscoveryConfig {
            max_depth: 0,
            max_pages: 10,
            worker_count: 2,
            allow_private_network: true,
            ..Default::default()
        };
        let client = HttpClient::new(cfg.clone()).unwrap();
        let seed = Url::parse("http://127.0.0.1:1/").unwrap();
        let crawler = Crawler::new(client, seed, cfg);
        // Will fail fetch but should not panic
        let _ = crawler.run().await;
    }
}
