//! `POST /properties/<id>/enrich` — kick off automated enrichment by enqueuing
//! an orchestrator job on the durable queue, which fans out into one job per
//! requested source.

use super::dto::{EnrichReq, EnrichResp};
use crate::auth::AuthUser;
use crate::enrichment::{Source, ORCHESTRATOR_KIND};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `POST /properties/<id>/enrich` — schedule enrichment for some/all sources.
#[rocket_okapi::openapi(tag = "Property Intelligence")]
#[post("/properties/<id>/enrich", data = "<body>")]
pub async fn enrich(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<EnrichReq>,
) -> ApiResult<Json<EnrichResp>> {
    user.require(Permission::PropertyWrite)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "property_intel").await?;

    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    // Resolve the requested sources (default: all), rejecting unknown keys.
    let req = body.into_inner();
    let scheduled: Vec<String> = if req.sources.is_empty() {
        Source::all()
            .iter()
            .map(|s| s.as_str().to_string())
            .collect()
    } else {
        let mut out = Vec::new();
        for s in &req.sources {
            let src = Source::from_str(s)
                .ok_or_else(|| ApiError::BadRequest(format!("unknown source: {s}")))?;
            out.push(src.as_str().to_string());
        }
        out
    };

    let job_id = scheduler::enqueue(
        &state.db,
        scope.tenant_id,
        ORCHESTRATOR_KIND,
        serde_json::json!({ "property_id": pid.to_string(), "sources": scheduled }),
        0,
    )
    .await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_ENRICH,
        Some("property"),
        Some(pid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "sources": scheduled, "job_id": job_id })),
    )
    .await;

    Ok(Json(EnrichResp { job_id, scheduled }))
}
