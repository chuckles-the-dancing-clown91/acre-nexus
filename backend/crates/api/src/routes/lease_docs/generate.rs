//! `POST /leases/<id>/document/generate` — render a lease agreement from the
//! tenant's legal templates + the concrete lease, its charges, and vehicles.

use super::dto::LeaseDocDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::leasedoc;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Lease, LeaseCharge, Property, Theme, Unit, Vehicle};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// `POST /leases/<id>/document/generate` — produce a new draft lease document.
#[rocket_okapi::openapi(tag = "Lease Documents")]
#[post("/leases/<id>/document/generate")]
pub async fn generate(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LeaseDocDto>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let property = Property::find_by_id(lease.property_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let unit = match lease.unit_id {
        Some(uid) => Unit::find_by_id(uid).one(&db).await?,
        None => None,
    };
    let charges = LeaseCharge::find()
        .filter(entity::lease_charge::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_charge::Column::LeaseId.eq(lid))
        .order_by_asc(entity::lease_charge::Column::CreatedAt)
        .all(&db)
        .await?;
    let vehicles = Vehicle::find()
        .filter(entity::vehicle::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::vehicle::Column::LeaseId.eq(lid))
        .all(&db)
        .await?;
    let templates = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|t| t.legal_templates)
        .unwrap_or_else(|| serde_json::json!({}));

    let body = leasedoc::render(
        &templates,
        &lease,
        &property,
        unit.as_ref(),
        &charges,
        &vehicles,
    );

    let now = Utc::now();
    let saved = entity::lease_document::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lid),
        title: Set("Residential Lease Agreement".into()),
        body: Set(body),
        format: Set("text".into()),
        status: Set("draft".into()),
        generated_at: Set(now.into()),
        signed_at: Set(None),
        signed_by: Set(None),
        signed_hash: Set(None),
        signed_ip: Set(None),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_DOC_GENERATE,
        Some("lease_document"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lid })),
    )
    .await;
    Ok(Json(LeaseDocDto::from(saved)))
}
