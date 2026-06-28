//! SeaORM models for the **user** domain (`acre_user` database).
//!
//! Cross-domain references (e.g. `audit_log.tenant_id`, `api_token.tenant_id`)
//! are plain `Uuid` columns enforced by the application layer, never DB foreign
//! keys — so these models carry no SeaORM relations into other domains.

pub mod api_token;
pub mod audit_log;
pub mod background_job;
pub mod membership;
pub mod permission;
pub mod profile_type;
pub mod refresh_token;
pub mod role;
pub mod role_permission;
pub mod tenant;
pub mod tenant_module;
pub mod theme;
pub mod user;
pub mod user_profile;
pub mod user_role;
