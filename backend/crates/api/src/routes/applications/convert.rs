//! `POST /applications/<id>/convert-to-lease` — turn an approved application into a
//! draft lease. Copies the applicant's identity + attributes (pets, military),
//! re-links any vehicles captured during application, applies the conditional fee
//! schedule, and links the lease back to the application. The lease starts
//! `upcoming`; signing the generated document activates it.

use super::dto::ConvertReq;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::lease_charges::apply_fees::apply_to_lease;
use crate::routes::rentals::dto::LeaseDto;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Application, Lease, Property, Vehicle};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /applications/<id>/convert-to-lease` — create a lease from an application.
#[rocket_okapi::openapi(tag = "Applications")]
#[post("/applications/<id>/convert-to-lease", data = "<body>")]
pub async fn convert(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ConvertReq>,
) -> ApiResult<Json<LeaseDto>> {
    user.require(Permission::ApplicationWrite)?;
    user.require(Permission::LeaseManage)?;
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let app = Application::find_by_id(aid)
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("application not found".into()))?;
    if app.status != "Approved" {
        return Err(ApiError::BadRequest(
            "application must be Approved before converting to a lease".into(),
        ));
    }
    // The lease's property must belong to the tenant.
    Property::find_by_id(b.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    // Idempotency: never convert the same application twice (would duplicate the
    // lease and steal the first lease's vehicles).
    if Lease::find()
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease::Column::ApplicationId.eq(aid))
        .one(&db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(
            "this application has already been converted to a lease".into(),
        ));
    }

    let now = Utc::now();
    let lease_id = Uuid::new_v4();
    let start_date = b
        .start_date
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| app.move_in.clone());

    // The whole request runs inside one RLS-scoped transaction (see `crate::db`),
    // so `&db` already gives us atomicity here.
    let lease = entity::lease::ActiveModel {
        id: Set(lease_id),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(b.property_id),
        unit_id: Set(b.unit_id),
        application_id: Set(Some(aid)),
        tenant_name: Set(app.applicant_name.clone()),
        tenant_email: Set(Some(app.email.clone())),
        tenant_phone: Set(Some(app.phone.clone())),
        rent_cents: Set(b.rent_cents),
        deposit_cents: Set(b.deposit_cents),
        start_date: Set(start_date),
        end_date: Set(b.end_date.clone()),
        status: Set("upcoming".into()),
        payment_status: Set("current".into()),
        balance_cents: Set(0),
        has_pet: Set(app.has_pet),
        pet_details: Set(app.pet_details.clone()),
        is_military: Set(app.is_military),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // Re-link vehicles captured during application to the new lease (tenant-scoped
    // so a foreign vehicle carrying this application_id can't be pulled in).
    let app_vehicles = Vehicle::find()
        .filter(entity::vehicle::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::vehicle::Column::ApplicationId.eq(aid))
        .all(&db)
        .await?;
    for v in app_vehicles {
        let mut vm: entity::vehicle::ActiveModel = v.into();
        vm.lease_id = Set(Some(lease_id));
        vm.updated_at = Set(now.into());
        vm.update(&db).await?;
    }

    // Mark the application as leased so it can't be converted again / re-shown,
    // through the same validated transition machinery as every other change.
    super::apply_transition(
        &db,
        scope.tenant_id,
        Some(user.user_id),
        app,
        "Leased",
        Some("Converted to lease".into()),
    )
    .await?;

    // The advertised listing (if any) is now under contract.
    crate::listing_sync::mark_pending_on_convert(&db, scope.tenant_id, aid).await;

    // Auto-apply the conditional fee schedule (pet fee, military discount, …).
    let applied = apply_to_lease(&db, scope.tenant_id, &lease).await?;
    if !applied.is_empty() {
        // Same domain event the manual apply-fees route writes — auto-applied
        // charges are money and belong in the audit log.
        crate::audit::record(
            &db,
            Some(user.user_id),
            crate::audit::actions::LEASE_FEES_APPLY,
            Some("lease"),
            Some(lease_id.to_string()),
            Some(scope.tenant_id),
            Some(serde_json::json!({
                "applied": applied.len(),
                "trigger": "conversion",
            })),
        )
        .await;
    }

    // Generate the first draft of the lease agreement so the file is ready to
    // review and send for signature the moment conversion finishes. The
    // workspace setting picks the default; the request can override per call
    // (e.g. external paperwork for one deal).
    let generate_doc = match b.generate_document {
        Some(v) => v,
        None => {
            crate::settings::get_bool(
                &db,
                scope.tenant_id,
                crate::settings::APPLICATION_GENERATE_DOC_ON_CONVERT,
            )
            .await
        }
    };
    if generate_doc {
        crate::routes::lease_docs::generate::generate_for_lease(
            &db,
            scope.tenant_id,
            &lease,
            Some(user.user_id),
        )
        .await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::APPLICATION_CONVERT,
        Some("lease"),
        Some(lease_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "application_id": aid,
            "charges_applied": applied.len(),
            "document_generated": generate_doc,
        })),
    )
    .await;

    Ok(Json(LeaseDto::from(lease)))
}
