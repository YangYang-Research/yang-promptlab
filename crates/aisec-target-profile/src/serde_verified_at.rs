//! Accept `verifiedAt` from wizard JSON (RFC3339 string) and legacy DB rows (time array).

use serde::{Deserialize, Deserializer, Serializer};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn serialize<S>(value: &Option<OffsetDateTime>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    time::serde::rfc3339::option::serialize(value, serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<OffsetDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(raw) => OffsetDateTime::parse(&raw, &Rfc3339)
            .map(Some)
            .map_err(serde::de::Error::custom),
        serde_json::Value::Array(_) => serde_json::from_value(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(
            "verifiedAt must be null, RFC3339 string, or timestamp array",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct VerifiedAtField {
        #[serde(with = "crate::serde_verified_at")]
        verified_at: Option<OffsetDateTime>,
    }

    #[test]
    fn deserializes_rfc3339_string() {
        let parsed: VerifiedAtField =
            serde_json::from_str(r#"{"verifiedAt":"2025-06-13T12:00:00Z"}"#).expect("string");
        assert!(parsed.verified_at.is_some());
    }

    #[test]
    fn deserializes_legacy_array_format() {
        let dt = OffsetDateTime::now_utc();
        let array = serde_json::to_value(dt).expect("array");
        let parsed: VerifiedAtField = serde_json::from_value(serde_json::json!({
            "verifiedAt": array
        }))
        .expect("legacy array");
        assert!(parsed.verified_at.is_some());
    }
}
