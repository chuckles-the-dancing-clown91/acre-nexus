//! The one **live** enrichment provider: the U.S. Census Bureau geocoder.
//!
//! It is free and keyless, which makes it a clean way to prove real outbound
//! validation while every other source stays simulated. Failures are returned as
//! [`EnrichmentError`] so the scheduler retries with backoff (and ultimately
//! fails the job) rather than crashing.
//!
//! ## Networking
//! In this managed environment outbound HTTPS goes through an agent proxy that
//! MITMs TLS, so we trust its CA bundle when present (and otherwise fall back to
//! the system/native roots). The proxy itself is picked up from `HTTPS_PROXY`
//! automatically by `reqwest`.

use super::data::{err, EnrichmentError, GeoData};
use std::time::Duration;

const GEOCODER_URL: &str = "https://geocoding.geo.census.gov/geocoder/locations/onelineaddress";

/// Candidate locations for the agent proxy's CA bundle.
const CA_BUNDLE_PATHS: &[&str] = &["/root/.ccr/ca-bundle.crt"];

/// Geocode a one-line address into coordinates + a normalised matched address.
pub async fn geocode(address: &str) -> Result<GeoData, EnrichmentError> {
    let client = build_client()?;
    let resp = client
        .get(GEOCODER_URL)
        .query(&[
            ("address", address),
            ("benchmark", "Public_AR_Current"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|e| err(format!("geocoder request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(err(format!("geocoder returned HTTP {}", resp.status())));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| err(format!("geocoder returned invalid JSON: {e}")))?;

    let first = body["result"]["addressMatches"]
        .as_array()
        .and_then(|m| m.first())
        .ok_or_else(|| err("no geocoder match for address"))?;

    let coords = &first["coordinates"];
    let latitude = coords["y"]
        .as_f64()
        .ok_or_else(|| err("geocoder match missing latitude"))?;
    let longitude = coords["x"]
        .as_f64()
        .ok_or_else(|| err("geocoder match missing longitude"))?;
    let matched_address = first["matchedAddress"]
        .as_str()
        .unwrap_or(address)
        .to_string();

    Ok(GeoData {
        latitude,
        longitude,
        matched_address,
        accuracy: "rooftop".into(),
    })
}

/// Build an HTTPS client that works behind the agent proxy.
fn build_client() -> Result<reqwest::Client, EnrichmentError> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("acre-nexus-enrichment/0.1");

    for cert in proxy_ca_certificates() {
        builder = builder.add_root_certificate(cert);
    }

    builder
        .build()
        .map_err(|e| err(format!("failed to build HTTP client: {e}")))
}

/// Load any extra CA certificates needed to trust the proxy (best-effort).
fn proxy_ca_certificates() -> Vec<reqwest::Certificate> {
    let mut paths: Vec<String> = CA_BUNDLE_PATHS.iter().map(|s| s.to_string()).collect();
    if let Ok(p) = std::env::var("SSL_CERT_FILE") {
        paths.push(p);
    }

    let mut certs = Vec::new();
    for path in paths {
        if let Ok(pem) = std::fs::read(&path) {
            match reqwest::Certificate::from_pem_bundle(&pem) {
                Ok(bundle) => certs.extend(bundle),
                Err(e) => tracing::debug!("ignoring CA bundle {path}: {e}"),
            }
        }
    }
    certs
}
