//! `GET /leases/<id>/envelope` — the lease's most recent e-signature envelope
//! with its signers and full ESIGN/UETA audit trail.

use super::dto::EnvelopeDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::EsignEnvelope;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases/<id>/envelope` — latest envelope (signers + audit trail).
#[rocket_okapi::openapi(tag = "E-Signature")]
#[get("/leases/<id>/envelope")]
pub async fn get(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<EnvelopeDto>> {
    user.require(Permission::LeaseRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let envelope = EsignEnvelope::find()
        .filter(entity::esign_envelope::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::esign_envelope::Column::LeaseId.eq(lid))
        .order_by_desc(entity::esign_envelope::Column::CreatedAt)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("no envelope for this lease".into()))?;
    let signers = esign::envelope_signers(&db, scope.tenant_id, envelope.id).await?;
    let events = super::envelope_events(&db, scope.tenant_id, envelope.id).await?;
    Ok(Json(EnvelopeDto::build(envelope, signers, events)))
}
