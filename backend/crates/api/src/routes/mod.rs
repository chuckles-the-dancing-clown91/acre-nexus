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

pub mod accounting;
pub mod api_tokens;
pub mod applications;
pub mod assignments;
pub mod auth;
pub mod banking;
pub mod billing;
pub mod cap_table;
pub mod deals;
pub mod documents;
pub mod domains;
pub mod entities;
pub mod esign;
pub mod fees;
pub mod iam;
pub mod integrations;
pub mod leads;
pub mod lease_charges;
pub mod lease_docs;
pub mod lifecycle;
pub mod listings;
pub mod llcs;
pub mod maintenance;
pub mod messages;
pub mod modules;
pub mod mortgages;
pub mod notifications;
pub mod onboarding;
pub mod payables;
pub mod payments;
pub mod payouts;
pub mod platform;
pub mod portfolio;
pub mod portfolios;
pub mod properties;
pub mod property_intel;
pub mod public;
pub mod rehab;
pub mod reminders;
pub mod rentals;
pub mod reports;
pub mod search;
pub mod settings;
pub mod tenant_history;
pub mod theme;
pub mod title;
pub mod vehicles;
pub mod vendor;
pub mod workflow;

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
        auth::login::login,
        auth::refresh::refresh,
        auth::me::me,
        auth::logout::logout,
        auth::workspaces::workspaces,
        auth::switch_workspace::switch_workspace,
        // platform (staff, cross-tenant)
        platform::tenants::tenants,
        platform::metrics::metrics,
        platform::staff::staff,
        platform::provision::provision,
        platform::impersonate::impersonate,
        platform::impersonations::list_impersonations,
        platform::impersonations::revoke_impersonation,
        // SaaS billing — platform plane (staff, cross-tenant)
        platform::billing::overview,
        platform::billing::invoices,
        platform::billing::run,
        platform::billing::mark_paid,
        platform::billing::void,
        platform::billing::set_plan,
        // SaaS billing — workspace self-serve
        billing::subscription::subscription,
        billing::invoices::list,
        billing::invoices::get,
        billing::invoices::export_invoice,
        // public routing entrypoint (host -> tenant + audience + theme)
        domains::resolve::resolve,
        // module management (tenant software settings)
        modules::list::list,
        modules::set::set,
        // per-tenant system settings
        settings::list::list,
        settings::set::set,
        // IAM — Acre admin: users, profiles/PII, roles, permissions, memberships
        iam::permissions::permissions,
        iam::profile_types::profile_types,
        iam::list_audit::list_audit,
        iam::list_roles::list_roles,
        iam::create_role::create_role,
        iam::update_role::update_role,
        iam::delete_role::delete_role,
        iam::list_users::list_users,
        iam::create_user::create_user,
        iam::get_user::get_user,
        iam::update_user::update_user,
        iam::put_profile::put_profile,
        iam::reveal_pii::reveal_pii,
        iam::add_membership::add_membership,
        iam::remove_membership::remove_membership,
        iam::assign_role::assign_role,
        iam::revoke_role::revoke_role,
        // IAM — tenant member management (client admins)
        iam::list_members::list_members,
        iam::invite_member::invite_member,
        // Self-service profile (renter portal / any signed-in user)
        iam::self_profile::my_profile,
        iam::self_profile::update_my_profile,
    ]
}
