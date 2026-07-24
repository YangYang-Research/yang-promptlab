use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use aisec_core::{AisecError, AisecResult};
use url::Url;

use crate::config::DiscoveryConfig;

/// Validate a URL is safe to fetch under the given policy.
pub fn validate_target_url(raw: &str, config: &DiscoveryConfig) -> AisecResult<Url> {
    let url = Url::parse(raw).map_err(|err| AisecError::invalid_input(format!("invalid URL: {err}")))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AisecError::invalid_input(format!(
                "unsupported scheme '{other}'; only http/https allowed"
            )));
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| AisecError::invalid_input("URL must have a host"))?;

    if !config.allow_private_network && is_blocked_host(host) {
        return Err(AisecError::invalid_input(format!(
            "host '{host}' resolves to a blocked network range; set allow_private_network=true to override"
        )));
    }

    Ok(url)
}

pub fn normalize_url(base: &Url, href: &str) -> Option<Url> {
    Url::parse(href)
        .ok()
        .or_else(|| base.join(href).ok())
        .map(|mut url| {
            url.set_fragment(None);
            url
        })
}

pub fn is_same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme() && a.host() == b.host() && a.port_or_known_default() == b.port_or_known_default()
}

pub fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

pub fn canonical_key(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    if (normalized.scheme() == "http" && normalized.port_or_known_default() == Some(80))
        || (normalized.scheme() == "https" && normalized.port_or_known_default() == Some(443))
    {
        let _ = normalized.set_port(None);
    }
    normalized.to_string()
}

fn is_blocked_host(host: &str) -> bool {
    let lower = host.to_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") || lower.ends_with(".local") {
        return true;
    }

    if let Ok(ip) = IpAddr::from_str(host) {
        return is_blocked_ip(ip);
    }

    false
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(v6) => is_blocked_ipv6(v6),
    }
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.octets()[0] == 169 && ip.octets()[1] == 254 // link-local APIPA
        || ip.octets()[0] == 0
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || (ip.segments()[0] & 0xfe00) == 0xfc00 // unique local
        || (ip.segments()[0] & 0xffc0) == 0xfe80 // link-local
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_localhost_by_default() {
        let cfg = DiscoveryConfig::default();
        assert!(validate_target_url("http://localhost/", &cfg).is_err());
        assert!(validate_target_url("http://127.0.0.1/", &cfg).is_err());
    }

    #[test]
    fn allows_private_when_configured() {
        let cfg = DiscoveryConfig {
            allow_private_network: true,
            ..Default::default()
        };
        assert!(validate_target_url("http://127.0.0.1:8080/", &cfg).is_ok());
    }

    #[test]
    fn resolves_relative_links() {
        let base = Url::parse("https://example.com/app/").unwrap();
        let resolved = normalize_url(&base, "/api/v1").unwrap();
        assert_eq!(resolved.as_str(), "https://example.com/api/v1");
    }
}
