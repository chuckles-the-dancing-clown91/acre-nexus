//! Vendor API-token management. Tenants create scoped keys to access the
//! `/api/v1` vendor API or resell services. The raw secret is returned **once**.

pub mod create;
pub mod dto;
pub mod list;
pub mod revoke;
