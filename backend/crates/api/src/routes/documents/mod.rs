//! Document-service routes (issue #17): upload (signed PUT URL), list by
//! owner, download (signed GET URL), delete, plus the local object-store blob
//! endpoints that back dev/CI. Mounted by the `integrations` module.

pub mod blob;
pub mod delete;
pub mod download;
pub mod dto;
pub mod list;
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

/// Upper bound we accept for a single stored file.
pub const MAX_SIZE_BYTES: i64 = 25 * 1024 * 1024;
