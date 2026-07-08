//! The one **live** enrichment provider: the U.S. Census Bureau geocoder.
//!
//! It is free and keyless, which makes it a clean way to prove real outbound
//! validation while every other source stays simulated. We call the
//! **geographies** endpoint, which returns coordinates *and* the real county +
//! county FIPS in a single request — so a live geocode enriches genuine
//! government data, not just a lat/long.
//!
//! Failures are returned as [`EnrichmentError`]; the [`super::runner`] catches
//! them and **falls back to a deterministic simulated geocode** so enrichment
//! still succeeds when the provider is unavailable (roadmap Phase 7 DoD).
//!
//! ## Networking
//! In this managed environment outbound HTTPS goes through an agent proxy that
//! MITMs TLS, so we trust its CA bundle when present (and otherwise fall back to
//! the system/native roots). The proxy itself is picked up from `HTTPS_PROXY`
//! automatically by `reqwest`.

use super::data::{err, EnrichmentError, GeoData};
use std::time::Duration;

const GEOCODER_URL: &str = "https://geocoding.geo.census.gov/geocoder/geographies/onelineaddress";

/// Candidate locations for the agent proxy's CA bundle.
const CA_BUNDLE_PATHS: &[&str] = &["/root/.ccr/ca-bundle.crt"];

/// Geocode a one-line address into coordinates + a normalised matched address +
/// (when the geographies layer resolves) the real county and county FIPS.
pub async fn geocode(address: &str) -> Result<GeoData, EnrichmentError> {
    let client = build_client()?;
    let resp = client
        .get(GEOCODER_URL)
        .query(&[
            ("address", address),
            ("benchmark", "Public_AR_Current"),
            ("vintage", "Current_Current"),
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

    parse_geocode(&body, address)
}

/// Pure parser over the Census geographies response — split out so it can be
/// unit-tested without a network call.
fn parse_geocode(body: &serde_json::Value, address: &str) -> Result<GeoData, EnrichmentError> {
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

    // The county layer carries the human name + the 5-digit county FIPS (GEOID).
    let county_layer = first["geographies"]["Counties"]
        .as_array()
        .and_then(|c| c.first());
    let county = county_layer
        .and_then(|c| c["NAME"].as_str())
        .map(|s| s.to_string());
    let fips = county_layer
        .and_then(|c| c["GEOID"].as_str())
        .map(|s| s.to_string());

    Ok(GeoData {
        latitude,
        longitude,
        matched_address,
        accuracy: "rooftop".into(),
        county,
        fips,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_coords_county_and_fips() {
        let body = json!({
            "result": { "addressMatches": [{
                "matchedAddress": "1600 PENNSYLVANIA AVE NW, WASHINGTON, DC, 20500",
                "coordinates": { "x": -77.0365, "y": 38.8977 },
                "geographies": { "Counties": [{
                    "NAME": "District of Columbia",
                    "GEOID": "11001",
                    "STATE": "11",
                    "COUNTY": "001"
                }]}
            }]}
        });
        let g = parse_geocode(&body, "input").unwrap();
        assert!((g.latitude - 38.8977).abs() < 1e-6);
        assert!((g.longitude + 77.0365).abs() < 1e-6);
        assert_eq!(g.county.as_deref(), Some("District of Columbia"));
        assert_eq!(g.fips.as_deref(), Some("11001"));
    }

    #[test]
    fn coords_without_geographies_degrade_to_none() {
        let body = json!({
            "result": { "addressMatches": [{
                "matchedAddress": "somewhere",
                "coordinates": { "x": -120.0, "y": 40.0 }
            }]}
        });
        let g = parse_geocode(&body, "input").unwrap();
        assert_eq!(g.county, None);
        assert_eq!(g.fips, None);
    }

    #[test]
    fn no_match_is_an_error() {
        let body = json!({ "result": { "addressMatches": [] } });
        assert!(parse_geocode(&body, "input").is_err());
    }
}
