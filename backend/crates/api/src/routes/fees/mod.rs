//! **Fee schedule** endpoints — the landlord-configured catalog of conditional
//! fees, discounts, rebates, and amenities that auto-populate leases.

pub mod create;
pub mod delete;
pub mod dto;
pub mod list;
pub mod update;

/// Allowed `kind` values for a fee-schedule entry.
pub const KINDS: &[&str] = &["fee", "discount", "rebate", "amenity"];
/// Allowed `condition_type` values.
pub const CONDITIONS: &[&str] = &["manual", "always", "has_pet", "is_military", "has_vehicle"];
