//! Transport wrapper around harness delivery. Payload mutation lives on the
//! harness interceptor pipeline (`PluginHarnessInterceptor`).

use std::sync::Arc;

use promptlab_attack::{
    AttackResult, HarnessTransport, TargetTransport, TransportRequest, TransportResponse,
};
use promptlab_plugin_host::PluginManager;
use async_trait::async_trait;
use tauri::async_runtime::Mutex as AsyncMutex;

/// Production attack transport: harness delivery (plugins run inside factory interceptors).
#[derive(Clone)]
pub struct PluginAwareTransport {
    inner: HarnessTransport,
    _plugins: Arc<AsyncMutex<PluginManager>>,
}

impl PluginAwareTransport {
    pub fn new(inner: HarnessTransport, plugins: Arc<AsyncMutex<PluginManager>>) -> Self {
        Self {
            inner,
            _plugins: plugins,
        }
    }

    pub fn inner(&self) -> &HarnessTransport {
        &self.inner
    }

    pub fn into_inner(self) -> HarnessTransport {
        self.inner
    }

    pub fn with_cancel(mut self, cancel: promptlab_harness::CancelFlag) -> Self {
        self.inner = self.inner.with_cancel(cancel);
        self
    }
}

#[async_trait]
impl TargetTransport for PluginAwareTransport {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse> {
        self.inner.send(request).await
    }
}
