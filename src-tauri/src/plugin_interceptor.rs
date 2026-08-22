//! Harness interceptor that runs enabled attack plugins on pre/post execute.

use std::sync::Arc;

use async_trait::async_trait;
use promptlab_harness::{
    AttackRequest, HarnessInterceptor, HarnessResult, InterceptAction, NormalizedResponse,
};
use promptlab_plugin_host::{intercept_attack_post, intercept_attack_pre, PluginManager};
use tauri::async_runtime::Mutex as AsyncMutex;

pub struct PluginHarnessInterceptor {
    plugins: Arc<AsyncMutex<PluginManager>>,
}

impl PluginHarnessInterceptor {
    pub fn new(plugins: Arc<AsyncMutex<PluginManager>>) -> Self {
        Self { plugins }
    }
}

#[async_trait]
impl HarnessInterceptor for PluginHarnessInterceptor {
    async fn pre_execute(&self, request: &mut AttackRequest) -> HarnessResult<InterceptAction> {
        if !request.purpose.is_attack() {
            return Ok(InterceptAction::Continue);
        }
        let mut manager = self.plugins.lock().await;
        match intercept_attack_pre(&mut manager, request.payload.clone(), request.body.clone())
            .await
        {
            Ok(result) => {
                if let Some(reason) = result.deny {
                    return Ok(InterceptAction::Deny { reason });
                }
                request.payload = result.payload;
                if let Some(body) = result.body {
                    request.body = Some(body);
                }
                Ok(InterceptAction::Continue)
            }
            Err(_) => Ok(InterceptAction::Continue),
        }
    }

    async fn post_execute(
        &self,
        request: &AttackRequest,
        response: &mut NormalizedResponse,
    ) -> HarnessResult<()> {
        if !request.purpose.is_attack() {
            return Ok(());
        }
        let mut manager = self.plugins.lock().await;
        if let Ok(Some(content)) =
            intercept_attack_post(&mut manager, &response.content, &response.raw_response).await
        {
            response.content = content;
        }
        Ok(())
    }
}
