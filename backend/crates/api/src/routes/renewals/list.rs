//! `GET /leases/<id>/renewals` — the renewal history for a lease, each with its
//! signing envelope (signers + audit trail) so the console can track progress.

use super::dto::RenewalDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::routes::esign::dto::EnvelopeDto;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{EsignEnvelope, LeaseRenewal};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /leases/<id>/renewals` — renewals for a lease, newest first.
#[rocket_okapi::openapi(tag = "Lease Renewals")]
#[get("/leases/<id>/renewals")]
pub async fn list_renewals(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<RenewalDto>>> {
    user.require(Permission::LeaseRead)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let rows = LeaseRenewal::find()
        .filter(entity::lease_renewal::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_renewal::Column::LeaseId.eq(lid))
        .order_by_desc(entity::lease_renewal::Column::CreatedAt)
        .all(&db)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let envelope = match r.envelope_id {
            Some(eid) => {
                match EsignEnvelope::find_by_id(eid)
                    .filter(entity::esign_envelope::Column::TenantId.eq(scope.tenant_id))
                    .one(&db)
                    .await?
                {
                    Some(env) => {
                        let signers = esign::envelope_signers(&db, scope.tenant_id, env.id).await?;
                        let events =
                            crate::routes::esign::envelope_events(&db, scope.tenant_id, env.id)
                                .await?;
                        Some(EnvelopeDto::build(env, signers, events))
                    }
                    None => None,
                }
            }
            None => None,
        };
        out.push(RenewalDto::build(r, envelope));
    }
    Ok(Json(out))
}
