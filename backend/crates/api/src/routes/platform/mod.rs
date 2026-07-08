//! Platform (Acre HQ) admin endpoints — **staff only**, cross-tenant. These are
//! the SaaS-vendor's own console: client companies and platform metrics. Client
//! users can never reach these (gated by the `platform:admin` permission).

pub mod billing;
pub mod dto;
pub mod impersonate;
pub mod impersonations;
pub mod metrics;
pub mod provision;
pub mod staff;
pub mod tenants;
