//! **Global Search** module (roadmap Phase 8, issue #55) — a single search
//! endpoint spanning the tenant's properties, tenants, counterparties,
//! maintenance tickets, and LLCs. Introduces no permission of its own: each
//! result type self-gates on its existing read permission, so search never
//! reveals more than the caller could already list.

use super::{ModuleManifest, PlatformModule};
use crate::routes::search;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct SearchModule;

impl PlatformModule for SearchModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "search",
            name: "Global Search",
            description: "Cross-entity search across properties, tenants, \
                          entities, tickets, and LLCs.",
            permissions: &[],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![search::search]
    }
}
