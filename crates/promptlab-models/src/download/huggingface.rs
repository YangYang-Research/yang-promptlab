/// Build HuggingFace resolve URL for a model file.
pub fn huggingface_url(repo: &str, filename: &str, revision: Option<&str>) -> String {
    let revision = revision.unwrap_or("main");
    format!("https://huggingface.co/{repo}/resolve/{revision}/{filename}")
}

/// HuggingFace HTTP client helpers.
#[derive(Clone)]
pub struct HuggingFaceClient {
    client: reqwest::Client,
}

impl HuggingFaceClient {
    pub fn new() -> Self {
        let client = promptlab_core::build_http_client(
            promptlab_core::HttpClientOptions::default()
                .with_user_agent("promptlab-models/0.1")
                .with_redirect_limit(10)
                .with_connect_timeout(std::time::Duration::from_secs(60))
                .without_gzip(),
        )
        .unwrap_or_else(|err| {
            tracing::warn!(error = %err, "falling back to direct HTTP client for HuggingFace");
            reqwest::Client::builder()
                .user_agent("promptlab-models/0.1")
                .redirect(reqwest::redirect::Policy::limited(10))
                .connect_timeout(std::time::Duration::from_secs(60))
                .no_gzip()
                .build()
                .expect("reqwest client")
        });
        Self { client }
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
