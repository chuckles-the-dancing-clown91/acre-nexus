//! Public website endpoints — no authentication. The tenant is resolved from the
//! `X-Tenant` header or `?tenant=<slug>` so the same API powers every client's
//! white-label site (or an embedded iframe).

pub mod apply;
pub mod dto;
pub mod listing_detail;
pub mod listings;
pub mod public_theme;
