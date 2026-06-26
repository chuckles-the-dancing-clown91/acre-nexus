//! Tenant-resolution request guards.
//!
//! Every tenant-scoped query must be filtered by the active tenant. These guards
//! produce that id from the right source depending on the caller:
//!
//! * **Authenticated users** — their own `tenant_id` from the JWT. Platform staff
//!   (no tenant) may *impersonate* a tenant by passing an `X-Tenant` header
//!   (slug or uuid) — useful for the HQ "view as client" flow.
//! * **Public website visitors** — resolved from the `X-Tenant` header or
//!   `?tenant=<slug>` query param (no auth).

pub mod helpers;
pub mod public;
pub mod scope;

pub use public::PublicTenant;
pub use scope::TenantScope;
