//! Limits for lists that grow with the business. A list answers at most `max`
//! rows (`default` when the caller does not say), newest first, and takes a
//! `before` cursor (the `created_at` of the last row seen) to page further.

use crate::error::ApiError;
use chrono::{DateTime, FixedOffset};

/// The row count to fetch: the caller's ask, within `1..=max`.
pub fn limit(asked: Option<u64>, default: u64, max: u64) -> u64 {
    asked.unwrap_or(default).clamp(1, max)
}

/// Parse a `before` cursor (an RFC 3339 timestamp).
pub fn before(raw: Option<&str>) -> Result<Option<DateTime<FixedOffset>>, ApiError> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map(Some)
            .map_err(|_| ApiError::BadRequest("before must be an RFC 3339 timestamp".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_clamped() {
        assert_eq!(limit(None, 200, 500), 200);
        assert_eq!(limit(Some(0), 200, 500), 1);
        assert_eq!(limit(Some(9999), 200, 500), 500);
        assert_eq!(limit(Some(25), 200, 500), 25);
    }

    #[test]
    fn cursors_parse() {
        assert!(before(None).unwrap().is_none());
        assert!(before(Some("")).unwrap().is_none());
        assert!(before(Some("2026-10-01T10:00:00+00:00")).unwrap().is_some());
        assert!(before(Some("yesterday")).is_err());
    }
}
