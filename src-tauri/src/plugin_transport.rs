//! Transport wrapper that applies enabled attack plugins before delivery.

use std::sync::Arc;

use aisec_attack::{
    AttackResult, HarnessTransport, TargetTransport, TransportRequest, TransportResponse,
};
use aisec_plugin_host::{mutate_attack_payload, PluginManager};
use async_trait::async_trait;
use tauri::async_runtime::Mutex as AsyncMutex;

/// Production attack transport: harness delivery with optional plugin payload mutation.
#[derive(Clone)]
pub struct PluginAwareTransport {
    inner: HarnessTransport,
    plugins: Arc<AsyncMutex<PluginManager>>,
}

impl PluginAwareTransport {
    pub fn new(inner: HarnessTransport, plugins: Arc<AsyncMutex<PluginManager>>) -> Self {
        Self { inner, plugins }
    }

    pub fn inner(&self) -> &HarnessTransport {
        &self.inner
    }

    pub fn into_inner(self) -> HarnessTransport {
        self.inner
    }
}

#[async_trait]
impl TargetTransport for PluginAwareTransport {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse> {
        let mut req = request;
        if let Some(body) = req.body.clone() {
            let mut manager = self.plugins.lock().await;
            if let Ok(mutated) = mutate_attack_payload(&mut manager, &body).await {
                req.body = Some(mutated);
            }
        }
        self.inner.send(req).await
    }
}
