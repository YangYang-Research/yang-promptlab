//! Host-backed project creation tool for Yazg ReAct.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Project row returned after a successful create.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedProject {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Host implements persistence (SQLite / IPC).
#[async_trait]
pub trait CreateProjectTools: Send + Sync {
    async fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CreatedProject, String>;
}
