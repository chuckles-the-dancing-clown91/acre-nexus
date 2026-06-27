//! Token-based authentication for the **vendor API** (`/api/v1/...`).
//!
//! Vendors authenticate with a long-lived, scoped, revocable key
//! (`acre_live_<secret>`). Only a SHA-256 hash is stored; scope checks gate
//! access to individual resources so services can be sold à la carte.

pub mod mint;
pub mod principal;

pub use mint::{mint, TOKEN_PREFIX};
pub use principal::ApiPrincipal;
