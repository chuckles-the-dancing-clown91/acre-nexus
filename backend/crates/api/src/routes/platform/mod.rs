//! Platform (Acre HQ) admin endpoints — **staff only**, cross-tenant. These are
//! the SaaS-vendor's own console: client companies and platform metrics. Client
//! users can never reach these (gated by the `platform:admin` permission).

pub mod dto;
pub mod metrics;
pub mod tenants;
