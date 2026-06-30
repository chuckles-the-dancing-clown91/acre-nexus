//! **Lease document** endpoints — generate a templated lease agreement from the
//! tenant's `theme.legal_templates` + the lease/charges/vehicles, fetch the latest,
//! and capture a typed signature (which activates the tenancy).

pub mod dto;
pub mod generate;
pub mod get;
pub mod sign;
