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
pub mod llcs;
pub mod modules;
pub mod platform;
pub mod portfolio;
pub mod properties;
pub mod public;
pub mod theme;
pub mod vendor;

use rocket::serde::json::Json;
use rocket::{get, routes, Route};

/// `GET /health` — liveness probe.
#[get("/health")]
pub fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "acre-api" }))
}

/// Always-on routes, independent of any module. Feature routes are added
/// separately by [`crate::modules::registry`] at boot.
pub fn core() -> Vec<Route> {
    routes![
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
    ]
}
