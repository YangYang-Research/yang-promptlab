use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::error::{HarnessError, HarnessResult};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct SigV4Credentials<'a> {
    pub access_key_id: &'a str,
    pub secret_access_key: &'a str,
    pub session_token: Option<&'a str>,
    pub region: &'a str,
    pub service: &'a str,
}

/// Sign an HTTP request with AWS Signature Version 4.
pub fn sign_headers(
    method: &str,
    url: &url::Url,
    headers: &mut std::collections::HashMap<String, String>,
    body: &str,
    creds: SigV4Credentials<'_>,
    now: OffsetDateTime,
) -> HarnessResult<()> {
    let amz_date = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    let date_stamp = &amz_date[..8];
    let host = url.host_str().ok_or_else(|| HarnessError::config("bedrock URL missing host"))?;
    headers.insert("host".into(), host.to_string());
    headers.insert("x-amz-date".into(), amz_date.clone());
    if let Some(token) = creds.session_token.filter(|t| !t.is_empty()) {
        headers.insert("x-amz-security-token".into(), token.to_string());
    }

    let payload_hash = hex::encode(Sha256::digest(body.as_bytes()));
    headers.insert("x-amz-content-sha256".into(), payload_hash.clone());

    let mut signed_names: Vec<String> = headers
        .keys()
        .map(|k| k.to_ascii_lowercase())
        .collect();
    signed_names.sort();
    signed_names.dedup();

    let canonical_headers = signed_names
        .iter()
        .map(|name| {
            let value = headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.trim())
                .unwrap_or("");
            format!("{name}:{value}\n")
        })
        .collect::<String>();
    let signed_header_list = signed_names.join(";");

    let canonical_query = url.query().unwrap_or("");
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        url.path(),
        canonical_query,
        canonical_headers,
        signed_header_list,
        payload_hash
    );
    let canonical_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let credential_scope = format!(
        "{}/{}/{}/aws4_request",
        date_stamp, creds.region, creds.service
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date, credential_scope, canonical_hash
    );

    let signing_key = signing_key(
        creds.secret_access_key,
        date_stamp,
        creds.region,
        creds.service,
    )?;
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes())?);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        creds.access_key_id, credential_scope, signed_header_list, signature
    );
    headers.insert("authorization".into(), authorization);
    Ok(())
}

fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> HarnessResult<Vec<u8>> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes())?;
    let k_region = hmac_sha256(&k_date, region.as_bytes())?;
    let k_service = hmac_sha256(&k_region, service.as_bytes())?;
    hmac_sha256(&k_service, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> HarnessResult<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|err| HarnessError::config(format!("hmac key: {err}")))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn signs_get_example_from_aws_docs_shape() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());
        let url = url::Url::parse("https://bedrock-runtime.us-east-1.amazonaws.com/model/x/invoke")
            .unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_440_938_160).unwrap();
        assert_eq!(now.month(), Month::August);
        sign_headers(
            "POST",
            &url,
            &mut headers,
            "{}",
            SigV4Credentials {
                access_key_id: "AKID",
                secret_access_key: "secret",
                session_token: None,
                region: "us-east-1",
                service: "bedrock",
            },
            now,
        )
        .unwrap();
        assert!(headers.get("authorization").unwrap().starts_with("AWS4-HMAC-SHA256 "));
        assert!(headers.contains_key("x-amz-date"));
    }
}
