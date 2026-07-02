//! Identity & Access Management routes — the back-end for the Acre employee
//! dashboard and for client workspace member management.
//!
//! Two audiences share this code:
//! * **Acre staff** operate `/admin/*` (gated by `user:*`, `role:*`,
//!   `profile:*`, `member:manage`; platform admins hold all). They manage users,
//!   profiles (incl. sensitive PII), personas, roles, and permissions across any
//!   tenant.
//! * **Client admins** operate `/members*` scoped to their active tenant
//!   (gated by `member:manage` / `member:read`) to run their own landlords,
//!   back-office staff, leasing agents, etc.
//!
//! Roles and their permission grants are stored in the DB, so everything here is
//! editable at runtime — no redeploy to add a role or change a permission.

pub mod dto;
pub mod helpers;

pub mod add_membership;
pub mod assign_role;
pub mod create_role;
pub mod create_user;
pub mod delete_role;
pub mod get_user;
pub mod invite_member;
pub mod list_audit;
pub mod list_members;
pub mod list_roles;
pub mod list_users;
pub mod permissions;
pub mod profile_types;
pub mod put_profile;
pub mod remove_membership;
pub mod reveal_pii;
pub mod revoke_role;
pub mod self_profile;
pub mod update_role;
pub mod update_user;
