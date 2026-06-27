//! Which requests are excluded from per-request auditing.
//!
//! The audit feed is about activity against the API, so we drop pure
//! infrastructure noise: liveness probes, the generated OpenAPI doc, the
//! interactive explorers, and CORS preflight (`OPTIONS`). Everything else —
//! including reads — is recorded.

/// Returns `true` when a request should *not* be audited.
pub fn should_skip(method: &str, path: &str) -> bool {
    if method.eq_ignore_ascii_case("OPTIONS") {
        return true;
    }
    const SKIP_EXACT: &[&str] = &["/health", "/openapi.json", "/favicon.ico"];
    const SKIP_PREFIX: &[&str] = &["/swagger-ui", "/rapidoc"];
    SKIP_EXACT.contains(&path) || SKIP_PREFIX.iter().any(|p| path.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::should_skip;

    #[test]
    fn skips_infra_and_preflight() {
        assert!(should_skip("OPTIONS", "/properties"));
        assert!(should_skip("GET", "/health"));
        assert!(should_skip("GET", "/openapi.json"));
        assert!(should_skip("GET", "/swagger-ui/index.html"));
        assert!(should_skip("GET", "/rapidoc/"));
    }

    #[test]
    fn audits_real_endpoints() {
        assert!(!should_skip("GET", "/properties"));
        assert!(!should_skip("POST", "/auth/login"));
        assert!(!should_skip("GET", "/api/v1/listings"));
    }
}
