use super::context::PromptContext;
use super::template::{PromptId, PromptTemplate};
use crate::prompts::PromptRegistry;

pub struct PromptBuilder {
    id: PromptId,
    context: PromptContext,
}

impl PromptBuilder {
    pub fn new(id: PromptId) -> Self {
        Self {
            id,
            context: PromptContext::new(),
        }
    }

    pub fn var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context = self.context.with(key, value);
        self
    }

    pub fn build(self) -> PromptTemplate {
        let mut template = PromptRegistry::get(self.id);
        let pairs: Vec<(&str, &str)> = self
            .context
            .vars
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        template.user = template.render_user(&pairs);
        template
    }
}
