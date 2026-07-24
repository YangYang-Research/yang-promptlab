use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Percent-encode a single URI path segment per AWS SigV4 rules.
pub fn aws_uri_encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

/// Build the Bedrock Converse API path for SigV4 signing (single URI-encoding).
pub fn bedrock_converse_path(model_id: &str) -> String {
    format!(
        "/model/{}/converse",
        aws_uri_encode(model_id.trim())
    )
}

#[derive(Debug)]
pub struct BedrockSignedRequest {
    pub authorization: String,
    pub amz_date: String,
    pub payload_hash: String,
    pub session_token: Option<String>,
}

pub fn sign_bedrock_post(
    host: &str,
    path: &str,
    body: &str,
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    session_token: Option<&str>,
) -> Result<BedrockSignedRequest, String> {
    let access_key_id = access_key_id.trim();
    let secret_access_key = secret_access_key.trim();
    let region = region.trim();
    let host = host.trim();
    let path = if path.is_empty() { "/" } else { path };
    let session_token = session_token.map(str::trim).filter(|value| !value.is_empty());

    if access_key_id.is_empty() || secret_access_key.is_empty() {
        return Err("access key id and secret access key are required".into());
    }

    if access_key_id.starts_with("ASIA") && session_token.is_none() {
        return Err(
            "session token is required for temporary AWS credentials (access key starts with ASIA)"
                .into(),
        );
    }

    let now = time::OffsetDateTime::now_utc();
    let amz_date = now
        .format(
            &time::format_description::parse("[year][month][day]T[hour][minute][second]Z")
                .unwrap(),
        )
        .map_err(|e| e.to_string())?;
    let date_stamp = now
        .format(&time::format_description::parse("[year][month][day]").unwrap())
        .map_err(|e| e.to_string())?;

    let payload_hash = hex::encode(Sha256::digest(body.as_bytes()));
    let (canonical_headers, signed_headers) = if let Some(token) = session_token {
        (
            format!(
                "content-type:application/json\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\nx-amz-security-token:{token}\n"
            ),
            "content-type;host;x-amz-content-sha256;x-amz-date;x-amz-security-token",
        )
    } else {
        (
            format!(
                "content-type:application/json\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
            ),
            "content-type;host;x-amz-content-sha256;x-amz-date",
        )
    };
    let canonical_request = format!(
        "POST\n{path}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let credential_scope = format!("{date_stamp}/{region}/bedrock/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let signing_key = derive_signing_key(secret_access_key, &date_stamp, region, "bedrock");
    let signature = hex::encode(sign_hmac(&signing_key, string_to_sign.as_bytes()));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    Ok(BedrockSignedRequest {
        authorization,
        amz_date,
        payload_hash,
        session_token: session_token.map(str::to_string),
    })
}

fn derive_signing_key(secret: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = sign_hmac(format!("AWS4{secret}").as_bytes(), date_stamp.as_bytes());
    let k_region = sign_hmac(&k_date, region.as_bytes());
    let k_service = sign_hmac(&k_region, service.as_bytes());
    sign_hmac(&k_service, b"aws4_request")
}

fn sign_hmac(key: impl AsRef<[u8]>, data: impl AsRef<[u8]>) -> Vec<u8> {
    let mut mac =
        HmacSha256::new_from_slice(key.as_ref()).expect("HMAC accepts any key length");
    mac.update(data.as_ref());
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_colon_in_model_id() {
        assert_eq!(
            aws_uri_encode("global.anthropic.claude-haiku-4-5-20251001-v1:0"),
            "global.anthropic.claude-haiku-4-5-20251001-v1%3A0"
        );
    }

    #[test]
    fn builds_converse_path_with_encoded_model() {
        assert_eq!(
            bedrock_converse_path("global.anthropic.claude-haiku-4-5-20251001-v1:0"),
            "/model/global.anthropic.claude-haiku-4-5-20251001-v1%3A0/converse"
        );
    }

    #[test]
    fn requires_session_token_for_asia_credentials() {
        let err = sign_bedrock_post(
            "bedrock-runtime.ap-southeast-1.amazonaws.com",
            "/model/test/converse",
            "{}",
            "ASIATESTACCESSKEY",
            "secret",
            "ap-southeast-1",
            None,
        )
        .unwrap_err();
        assert!(err.contains("session token"));
    }

    #[test]
    fn includes_security_token_header_when_present() {
        let signed = sign_bedrock_post(
            "bedrock-runtime.ap-southeast-1.amazonaws.com",
            "/model/test/converse",
            "{}",
            "ASIATESTACCESSKEY",
            "secret",
            "ap-southeast-1",
            Some("session-token-value"),
        )
        .expect("sign");
        assert_eq!(
            signed.session_token.as_deref(),
            Some("session-token-value")
        );
        assert!(signed.authorization.contains("x-amz-security-token"));
    }

    /// Run manually: `AWS_ACCESS_KEY_ID=... AWS_SECRET_ACCESS_KEY=... AWS_SESSION_TOKEN=... \
    /// cargo test -p aisec-judge bedrock_live_converse -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "requires live AWS Bedrock credentials"]
    async fn bedrock_live_converse() {
        let access_key = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID");
        let secret = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY");
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let region = std::env::var("AWS_DEFAULT_REGION").unwrap_or_else(|_| "ap-southeast-1".into());
        let model_id = std::env::var("BEDROCK_MODEL_ID")
            .unwrap_or_else(|_| "global.anthropic.claude-haiku-4-5-20251001-v1:0".into());
        let host = format!("bedrock-runtime.{region}.amazonaws.com");
        let path = bedrock_converse_path(&model_id);
        let body = r#"{"messages":[{"role":"user","content":[{"text":"Say ok"}]}],"inferenceConfig":{"maxTokens":16,"temperature":0}}"#;
        let signed = sign_bedrock_post(
            &host,
            &path,
            body,
            &access_key,
            &secret,
            &region,
            session_token.as_deref(),
        )
        .expect("sign");
        let url = format!("https://{host}/model/{model_id}/converse");
        let mut request = reqwest::Client::new()
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", signed.authorization)
            .header("x-amz-date", signed.amz_date)
            .header("x-amz-content-sha256", signed.payload_hash);
        if let Some(token) = signed.session_token.as_deref() {
            request = request.header("x-amz-security-token", token);
        }
        let response = request.body(body).send().await.expect("request");
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        assert!(status.is_success(), "status={status} body={text}");
    }
}
