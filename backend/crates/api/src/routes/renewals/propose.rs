//! `POST /leases/<id>/renewals` — propose a renewal (new rent + term) on an
//! existing lease and generate the addendum the resident will e-sign.

use super::dto::{ProposeRenewalReq, ProposeRenewalResp, RenewalDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::{leasedoc, renewals};
use chrono::Utc;
use entity::prelude::{Lease, LeaseRenewal, Property, Unit};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /leases/<id>/renewals` — propose renewed terms + generate the addendum.
#[rocket_okapi::openapi(tag = "Lease Renewals")]
#[post("/leases/<id>/renewals", data = "<body>")]
pub async fn propose(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ProposeRenewalReq>,
) -> ApiResult<Json<ProposeRenewalResp>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    if matches!(lease.status.as_str(), "ended" | "expired") {
        return Err(ApiError::BadRequest(format!(
            "cannot renew a lease that is '{}' — create a new lease instead",
            lease.status
        )));
    }
    if b.new_rent_cents <= 0 {
        return Err(ApiError::BadRequest(
            "new_rent_cents must be greater than zero".into(),
        ));
    }
    if let Some(m) = b.term_months {
        if m <= 0 {
            return Err(ApiError::BadRequest(
                "term_months must be greater than zero".into(),
            ));
        }
    }

    // One in-flight renewal per lease — cancel the open one to re-propose.
    let open = LeaseRenewal::find()
        .filter(entity::lease_renewal::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_renewal::Column::LeaseId.eq(lid))
        .filter(entity::lease_renewal::Column::Status.is_in(renewals::OPEN_STATUSES.to_vec()))
        .one(&db)
        .await?;
    if open.is_some() {
        return Err(ApiError::Conflict(
            "a renewal is already in progress for this lease — cancel it first".into(),
        ));
    }

    // Effective start: the caller's date, else the day after the current term.
    let new_start_date = match b.new_start_date.filter(|d| !d.trim().is_empty()) {
        Some(d) => {
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map_err(|_| ApiError::BadRequest("new_start_date must be YYYY-MM-DD".into()))?;
            d
        }
        None => lease
            .end_date
            .as_deref()
            .and_then(renewals::day_after)
            .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string()),
    };

    // End: explicit date, else start + term_months, else month-to-month.
    let new_end_date = match b.new_end_date.filter(|d| !d.trim().is_empty()) {
        Some(d) => {
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map_err(|_| ApiError::BadRequest("new_end_date must be YYYY-MM-DD".into()))?;
            Some(d)
        }
        None => match b.term_months {
            Some(m) => Some(renewals::add_months_str(&new_start_date, m as u32).ok_or_else(
                || ApiError::BadRequest("could not compute the renewal end date".into()),
            )?),
            None => None,
        },
    };
    if let Some(end) = &new_end_date {
        if !renewals::end_after_start(&new_start_date, end) {
            return Err(ApiError::BadRequest(
                "the renewal end date must be after its start date".into(),
            ));
        }
    }

    let notes = b.notes.filter(|n| !n.trim().is_empty());
    let now = Utc::now();
    let renewal = entity::lease_renewal::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lid),
        status: Set("proposed".into()),
        current_rent_cents: Set(lease.rent_cents),
        new_rent_cents: Set(b.new_rent_cents),
        new_start_date: Set(new_start_date),
        new_end_date: Set(new_end_date),
        term_months: Set(b.term_months),
        notes: Set(notes),
        lease_document_id: Set(None),
        envelope_id: Set(None),
        created_by: Set(Some(user.user_id)),
        activated_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // Render + persist the addendum document (kept distinct from the lease
    // agreement by `purpose`, so the normal signing flow never picks it up).
    let property = Property::find_by_id(lease.property_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let unit = match lease.unit_id {
        Some(uid) => Unit::find_by_id(uid).one(&db).await?,
        None => None,
    };
    let body_text = leasedoc::render_renewal_addendum(&lease, &property, unit.as_ref(), &renewal);
    let title =
        crate::settings::get_string(&db, scope.tenant_id, crate::settings::LEASE_RENEWAL_DOC_TITLE)
            .await;
    let doc = entity::lease_document::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lid),
        title: Set(title),
        body: Set(body_text.clone()),
        format: Set("text".into()),
        purpose: Set("renewal_addendum".into()),
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

    let doc_id = doc.id;
    let mut rm: entity::lease_renewal::ActiveModel = renewal.into();
    rm.lease_document_id = Set(Some(doc_id));
    rm.updated_at = Set(now.into());
    let renewal = rm.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_RENEWAL_PROPOSE,
        Some("lease_renewal"),
        Some(renewal.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({
            "lease_id": lid,
            "current_rent_cents": renewal.current_rent_cents,
            "new_rent_cents": renewal.new_rent_cents,
            "new_end_date": renewal.new_end_date,
        })),
    )
    .await;

    Ok(Json(ProposeRenewalResp {
        renewal: RenewalDto::from(renewal),
        document_id: doc_id,
        document_body: body_text,
    }))
}
