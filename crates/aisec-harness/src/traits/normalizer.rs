use crate::error::HarnessResult;
use crate::models::{AttackRequest, NormalizedResponse};

/// Converts raw harness output into judge-ready normalized responses.
pub trait ResponseNormalizer: Send + Sync {
    fn normalize(&self, request: &AttackRequest, response: NormalizedResponse) -> HarnessResult<NormalizedResponse>;
}

/// Pass-through normalizer with lightweight content cleanup.
#[derive(Clone, Copy, Default)]
pub struct DefaultResponseNormalizer;

impl ResponseNormalizer for DefaultResponseNormalizer {
    fn normalize(
        &self,
        request: &AttackRequest,
        mut response: NormalizedResponse,
    ) -> HarnessResult<NormalizedResponse> {
        if response.content.trim().is_empty() {
            response.content = response.raw_response.clone();
        }
        response
            .metadata
            .entry("payload_length".into())
            .or_insert_with(|| request.payload.len().to_string());
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::NormalizedResponse;

    #[test]
    fn fills_empty_content_from_raw() {
        let request = AttackRequest::from_payload("https://example.com", "test");
        let normalized = NormalizedResponse {
            content: String::new(),
            raw_response: "raw-body".into(),
            status_code: Some(200),
            metadata: Default::default(),
        };
        let result = DefaultResponseNormalizer
            .normalize(&request, normalized)
            .unwrap();
        assert_eq!(result.content, "raw-body");
    }
}
