//! Role-based access control: the permission catalog, persona (profile-type)
//! catalog, and the seeded system roles that wire them together.
//!
//! ## Data-driven by design
//! Permissions are fine-grained `resource:action` strings persisted in
//! `role_permission` and resolved per user at login (see
//! [`crate::auth::permissions_for`]). The [`Permission`] enum below is a
//! compile-time convenience for the keys the Rust handlers check directly; the
//! Acre admin dashboard can additionally create roles and grant **any** catalog
//! permission at runtime without code changes.
//!
//! ## Three layers
//! * [`PERMISSION_CATALOG`] — every permission the platform ships, with UI
//!   metadata. Seeded into the `permission` table so the dashboard can offer a
//!   picker (and so custom permissions can be added later).
//! * [`PROFILE_TYPES`] — the personas a membership can have (Acre employees vs
//!   client landlords/back-office/renters …). Seeded into `profile_type`.
//! * [`SYSTEM_ROLES`] — default roles, each a named bundle of permissions at a
//!   `platform` or `tenant` scope. Seeded into `role` + `role_permission`.

pub mod catalog;
pub mod grants;
pub mod permission;
pub mod personas;
pub mod roles;
pub mod scope;

pub use catalog::PERMISSION_CATALOG;
pub use grants::Grants;
pub use permission::{Permission, SCOPE_PLATFORM, SCOPE_TENANT};
pub use personas::{default_role_for_persona, PROFILE_TYPES};
pub use roles::SYSTEM_ROLES;
#[allow(unused_imports)]
pub use scope::{scope_covers, ResourceScope};
