//! **Acquisitions & Flips** module — the house-flipping deal pipeline for
//! investor tenants. Shipped as a **preview**: it is off by default and a tenant
//! opts in from their software settings (`PATCH /modules/flips`).
//!
//! This module is the reference example of the plugin contract: it owns its
//! permission requirements, contributes its own route, and **self-gates** on
//! per-tenant enablement via [`super::require_enabled`]. Promoting it to GA is a
//! one-line change (`preview: false`, `default_enabled: true`); fleshing out the
//! domain means adding a `deal` entity + migration and richer routes here —
//! nothing elsewhere needs to change.

use super::{ModuleManifest, PlatformModule};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, Route, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use serde_json::json;

pub struct FlipsModule;

impl PlatformModule for FlipsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "flips",
            name: "Acquisitions & Flips",
            description: "Buy/flip deal pipeline with underwriting (preview).",
            permissions: &[Permission::PropertyRead, Permission::PropertyWrite],
            job_kinds: &[],
            default_enabled: false,
            preview: true,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![pipeline]
    }
}

/// `GET /modules/flips/pipeline` — the flip deal board. Requires `property:read`
/// **and** the flips module to be enabled for the active tenant.
#[rocket_okapi::openapi(tag = "Flips")]
#[get("/modules/flips/pipeline")]
pub async fn pipeline(
    state: &State<AppState>,
    user: AuthUser,
    tenant: TenantScope,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PropertyRead)?;
    super::require_enabled(&state.db, tenant.tenant_id, "flips").await?;

    // Preview scaffold: the stage taxonomy is real; deals are populated once the
    // `deal` domain lands. The board renders against this shape today.
    Ok(Json(json!({
        "preview": true,
        "stages": [
            { "key": "sourcing",    "label": "Sourcing" },
            { "key": "under_contract", "label": "Under contract" },
            { "key": "rehab",       "label": "Rehab" },
            { "key": "listed",      "label": "Listed" },
            { "key": "sold",        "label": "Sold" }
        ],
        "deals": []
    })))
}
