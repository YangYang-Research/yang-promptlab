use url::Url;

pub fn openapi_probe_paths(origin: &str) -> Vec<String> {
    const PATHS: &[&str] = &[
        "/openapi.json",
        "/openapi.yaml",
        "/openapi.yml",
        "/swagger.json",
        "/swagger/v1/swagger.json",
        "/api-docs",
        "/api/openapi.json",
        "/api/swagger.json",
        "/v2/api-docs",
        "/v3/api-docs",
        "/docs/openapi.json",
        "/.well-known/openapi.json",
    ];
    join_paths(origin, PATHS)
}

pub fn graphql_probe_paths(origin: &str) -> Vec<String> {
    const PATHS: &[&str] = &[
        "/graphql",
        "/api/graphql",
        "/v1/graphql",
        "/query",
        "/gql",
        "/graphiql",
    ];
    join_paths(origin, PATHS)
}

pub fn ai_probe_paths(origin: &str) -> Vec<String> {
    const PATHS: &[&str] = &[
        "/v1/chat/completions",
        "/v1/completions",
        "/v1/embeddings",
        "/v1/models",
        "/api/chat",
        "/api/generate",
        "/api/v1/chat/completions",
        "/api/llm",
        "/api/ai/chat",
        "/anthropic/v1/messages",
        "/openai/deployments",
        "/predict",
        "/invoke",
        "/api/inference",
    ];
    join_paths(origin, PATHS)
}

fn join_paths(origin: &str, paths: &[&str]) -> Vec<String> {
    let base = Url::parse(origin).ok();
    paths
        .iter()
        .filter_map(|path| {
            base.as_ref()
                .and_then(|b| b.join(path).ok())
                .map(|u| u.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_openapi_probe_urls() {
        let urls = openapi_probe_paths("https://example.com");
        assert!(urls.iter().any(|u| u.ends_with("/openapi.json")));
    }
}
