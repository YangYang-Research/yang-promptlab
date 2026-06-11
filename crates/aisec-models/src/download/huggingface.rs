/// Build HuggingFace resolve URL for a model file.
pub fn huggingface_url(repo: &str, filename: &str, revision: Option<&str>) -> String {
    let revision = revision.unwrap_or("main");
    format!("https://huggingface.co/{repo}/resolve/{revision}/{filename}")
}

/// HuggingFace HTTP client helpers.
pub struct HuggingFaceClient {
    client: reqwest::Client,
}

impl HuggingFaceClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("aisec-models/0.1")
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn resolve_url(
        &self,
        repo: &str,
        filename: &str,
        revision: Option<&str>,
    ) -> String {
        huggingface_url(repo, filename, revision)
    }
}

impl Default for HuggingFaceClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_resolve_url() {
        let url = huggingface_url(
            "TheBloke/Llama-2-7B-GGUF",
            "llama-2-7b.Q4_K_M.gguf",
            Some("main"),
        );
        assert!(url.contains("huggingface.co"));
        assert!(url.contains("resolve/main"));
    }
}
