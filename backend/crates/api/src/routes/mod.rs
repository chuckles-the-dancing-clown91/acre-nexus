//! HTTP route modules.
//!
//! Routes split into two tiers:
//! * **Core routes** ([`core`]) are always mounted — health, auth, the
//!   staff-only platform admin, and module management.
//! * **Feature routes** are owned by pluggable modules (see [`crate::modules`])
//!   and mounted per module at boot, so a tenant's enabled feature set is
//!   composable rather than hard-wired here.
//!
//! The audience-specific handlers below remain organised by area; modules
//! reference them (e.g. [`properties`] is wrapped by `modules::properties`).

pub mod api_tokens;
pub mod applications;
pub mod auth;
pub mod iam;
pub mod llcs;
pub mod modules;
pub mod platform;
pub mod portfolio;
pub mod properties;
pub mod public;
pub mod theme;
pub mod vendor;

use rocket::serde::json::Json;
use rocket::{get, Route};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

/// `GET /health` — liveness probe.
#[rocket_okapi::openapi(tag = "System")]
#[get("/health")]
pub fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "acre-api" }))
}

/// Always-on routes, independent of any module, paired with their OpenAPI spec.
/// Feature routes are added separately by [`crate::modules::registry`] at boot.
pub fn core_api() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        health,
        // auth
        auth::login,
        auth::refresh,
        auth::me,
        auth::logout,
        // platform (staff, cross-tenant)
        platform::tenants,
        platform::metrics,
        // module management (tenant software settings)
        modules::list,
        modules::set,
        // IAM — Acre admin: users, profiles/PII, roles, permissions, memberships
        iam::permissions,
        iam::profile_types,
        iam::list_roles,
        iam::create_role,
        iam::update_role,
        iam::delete_role,
        iam::list_users,
        iam::create_user,
        iam::get_user,
        iam::update_user,
        iam::put_profile,
        iam::reveal_pii,
        iam::add_membership,
        iam::remove_membership,
        iam::assign_role,
        iam::revoke_role,
        // IAM — tenant member management (client admins)
        iam::list_members,
        iam::invite_member,
    ]
}
