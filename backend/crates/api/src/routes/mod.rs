//! HTTP route modules, grouped by audience:
//! * [`auth`] — login / refresh / me / logout
//! * [`public`] — unauthenticated white-label website (listings, applications)
//! * [`properties`], [`llcs`], [`portfolio`], [`applications`], [`theme`],
//!   [`api_tokens`] — authenticated, tenant-scoped landlord/PM console
//! * [`platform`] — staff-only cross-tenant admin
//! * [`vendor`] — token-authenticated `/api/v1` vendor API

pub mod api_tokens;
pub mod applications;
pub mod auth;
pub mod llcs;
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

/// All routes mounted under the API root.
pub fn all() -> Vec<Route> {
    routes![
        health,
        // auth
        auth::login,
        auth::refresh,
        auth::me,
        auth::logout,
        // public website
        public::listings,
        public::listing_detail,
        public::public_theme,
        public::apply,
        // landlord / PM console
        properties::list,
        properties::create,
        properties::profile,
        properties::update,
        llcs::list,
        llcs::create,
        portfolio::summary,
        portfolio::llc_groups,
        applications::list,
        applications::update_status,
        theme::get_theme,
        theme::update_theme,
        api_tokens::list,
        api_tokens::create,
        api_tokens::revoke,
        // platform (staff)
        platform::tenants,
        platform::metrics,
        // vendor API
        vendor::listings,
        vendor::properties,
    ]
}
