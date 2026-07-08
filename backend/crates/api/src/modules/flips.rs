//! **Acquisitions & Flips** module — the buy-side deal pipeline + underwriting
//! for investor tenants (roadmap Phase 7, issues #41/#42).
//!
//! A [`deal`](entity::deal) moves through the acquisition pipeline
//! (`prospecting → offer → under_contract → closing → owned`), carries its
//! underwriting assumptions (cap rate / cash-on-cash / IRR / DSCR + sensitivity)
//! and a due-diligence checklist, keeps its supporting files in the polymorphic
//! [`document`](entity::document) data room (`owner_type = "deal"`), and converts
//! into a fully-onboarded [`property`](entity::property).
//!
//! Still the reference example of the plugin contract: it owns its permissions
//! (`deal:read` / `deal:write`), contributes its own routes, and **self-gates**
//! on per-tenant enablement via [`super::require_enabled`].

use super::{ModuleManifest, PlatformModule};
use crate::auth::AuthUser;
use crate::deals::DEAL_STAGES;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::routes::deals::dto::DealDto;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Deal;
use rocket::serde::json::Json;
use rocket::{get, Route, State};
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::json;

pub struct FlipsModule;

impl PlatformModule for FlipsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "flips",
            name: "Acquisitions & Flips",
            description: "Buy-side deal pipeline with underwriting (cap rate, \
                          cash-on-cash, IRR, DSCR), a due-diligence data room, \
                          and one-click conversion into an owned property.",
            permissions: &[Permission::DealRead, Permission::DealWrite],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        use crate::routes::deals;
        openapi_get_routes_spec![
            pipeline,
            deals::list::list,
            deals::create::create,
            deals::get::get,
            deals::update::update,
            deals::advance::advance,
            deals::underwrite::underwrite_deal,
            deals::checklist::update_checklist,
            deals::convert::convert,
        ]
    }
}

/// `GET /modules/flips/pipeline` — the acquisition board: the stage taxonomy
/// plus every deal (with computed underwriting), for the console to lay out as
/// kanban columns. Requires `deal:read` **and** the flips module enabled.
#[rocket_okapi::openapi(tag = "Flips")]
#[get("/modules/flips/pipeline")]
pub async fn pipeline(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    tenant: TenantScope,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::DealRead)?;
    super::require_enabled(&state.db, tenant.tenant_id, "flips").await?;

    let rows = Deal::find()
        .filter(entity::deal::Column::TenantId.eq(tenant.tenant_id))
        .order_by_desc(entity::deal::Column::CreatedAt)
        .all(&db)
        .await?;
    let deals: Vec<DealDto> = rows.iter().map(DealDto::build).collect();
    let stages: Vec<_> = DEAL_STAGES
        .iter()
        .map(|s| json!({ "key": s.key, "label": s.label }))
        .collect();

    Ok(Json(json!({
        "preview": false,
        "stages": stages,
        "deals": deals,
    })))
}
