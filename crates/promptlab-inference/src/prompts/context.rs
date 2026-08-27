#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    pub vars: Vec<(String, String)>,
}

impl PromptContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.push((key.into(), value.into()));
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}
