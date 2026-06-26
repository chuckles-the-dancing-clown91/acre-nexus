//! Landlord / property-manager property endpoints (tenant-scoped, RBAC-gated).

pub mod create;
pub mod dto;
pub mod list;
pub mod profile;
pub mod update;

pub use dto::PropertyResp;
