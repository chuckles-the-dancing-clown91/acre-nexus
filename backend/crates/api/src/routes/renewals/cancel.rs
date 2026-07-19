//! `POST /renewals/<id>/cancel` — withdraw an in-flight renewal, voiding any
//! open signing envelope so the emailed links die.

use super::dto::RenewalDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::renewals;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::LeaseRenewal;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /renewals/<id>/cancel` — cancel a proposed/sent renewal.
#[rocket_okapi::openapi(tag = "Lease Renewals")]
#[post("/renewals/<id>/cancel")]
pub async fn cancel(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<RenewalDto>> {
    user.require(Permission::LeaseManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let renewal = LeaseRenewal::find_by_id(rid)
        .filter(entity::lease_renewal::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("renewal not found".into()))?;
    if !renewals::OPEN_STATUSES.contains(&renewal.status.as_str()) {
        return Err(ApiError::Conflict(format!(
            "renewal is '{}' — only an in-flight renewal can be cancelled",
            renewal.status
        )));
    }

    // Kill any open envelope on the addendum so the signing links stop working.
    if let Some(doc_id) = renewal.lease_document_id {
        esign::void_open_envelopes_for_document(
            &db,
            scope.tenant_id,
            doc_id,
            "lease renewal cancelled",
        )
        .await?;
    }

    let now = Utc::now();
    let renewal_lease = renewal.lease_id;
    let mut rm: entity::lease_renewal::ActiveModel = renewal.into();
    rm.status = Set("cancelled".into());
    rm.updated_at = Set(now.into());
    let renewal = rm.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_RENEWAL_CANCEL,
        Some("lease_renewal"),
        Some(renewal.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "lease_id": renewal_lease })),
    )
    .await;

    Ok(Json(RenewalDto::from(renewal)))
}
