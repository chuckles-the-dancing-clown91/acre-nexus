//! Document-service routes (issue #17): upload (signed PUT URL), list by
//! owner, download (signed GET URL), delete, plus the local object-store blob
//! endpoints that back dev/CI. Mounted by the `integrations` module.

pub mod blob;
pub mod delete;
pub mod download;
pub mod dto;
pub mod list;
pub mod property;
pub mod update;
pub mod upload;

/// Record kinds a document may attach to.
pub const OWNER_TYPES: &[&str] = &[
    "property",
    "lease",
    "application",
    "entity",
    "deal",
    "unit",
    "maintenance_ticket",
    "tenant",
];

/// Filing buckets for the documents tab. Free-form is tolerated (the column is
/// nullable text), but the upload/patch paths normalise to this catalog.
pub const CATEGORIES: &[&str] = &[
    "insurance",
    "loan",
    "title",
    "tax",
    "lease",
    "inspection",
    "permit",
    "receipt",
    "statement",
    "notice",
    "other",
];

/// Normalise a caller-supplied category to a trimmed lowercase value, rejecting
/// anything outside [`CATEGORIES`]. `None`/empty clears it.
pub fn normalize_category(raw: Option<String>) -> Result<Option<String>, String> {
    match raw {
        None => Ok(None),
        Some(s) => {
            let s = s.trim().to_lowercase();
            if s.is_empty() {
                Ok(None)
            } else if CATEGORIES.contains(&s.as_str()) {
                Ok(Some(s))
            } else {
                Err(format!(
                    "invalid category: {s} (expected one of {})",
                    CATEGORIES.join(", ")
                ))
            }
        }
    }
}

/// Upper bound we accept for a single stored file.
pub const MAX_SIZE_BYTES: i64 = 25 * 1024 * 1024;
