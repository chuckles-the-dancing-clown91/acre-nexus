//! `POST /public/applications` — the anonymous website intake door.
//!
//! One of three doors into the same application pipeline (see
//! [`crate::routes::applications::intake`]); the others are the renter
//! portal (`POST /my/applications`) and back-office intake
//! (`POST /applications`).

use super::dto::{ApplyReq, ApplyResp};
use crate::error::ApiResult;
use crate::routes::applications::{intake, IntakeInput};
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

/// `POST /public/applications` — submit a rental application.
///
/// Persists the application and enqueues a background-screening job that the
/// Tokio scheduler advances asynchronously; the screening outcome lands back
/// on the application (and can auto-approve, per the workspace setting).
#[rocket_okapi::openapi(tag = "Public Website")]
#[post("/public/applications", data = "<body>")]
pub async fn apply(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    body: Json<ApplyReq>,
) -> ApiResult<Json<ApplyResp>> {
    let b = body.into_inner();
    let email = b.email.trim().to_lowercase();
    // Before anything is sent to them, so the receipt is in their language.
    if let Some(lang) = b.language.as_deref().and_then(crate::language::parse) {
        crate::language::set(
            &db,
            tenant.tenant_id,
            &[Some(&email), b.phone.as_deref()],
            lang,
        )
        .await?;
    }

    // Reuse: if the workspace allows it and this applicant already has a recent
    // *approved* application, carry that screening result forward — the new
    // application is pre-approved for this listing and skips re-screening.
    let reused_from =
        crate::routes::applications::reuse::latest_reusable_approved(&db, tenant.tenant_id, &email)
            .await?;

    let (saved, job_id) = intake(
        &db,
        tenant.tenant_id,
        IntakeInput {
            listing_id: b.listing_id,
            applicant_name: b.applicant_name,
            email,
            phone: b.phone.unwrap_or_default(),
            annual_income_cents: b.annual_income_cents.unwrap_or(0),
            credit_score: b.credit_score,
            move_in: b.move_in.unwrap_or_default(),
            has_pet: b.has_pet.unwrap_or(false),
            pet_details: b.pet_details,
            is_military: b.is_military.unwrap_or(false),
            screening_consent: b.screening_consent.unwrap_or(false),
        },
        "public",
        None,
        None,
        reused_from.as_ref(),
    )
    .await?;

    // An invited prospect: the lead now has its application.
    if let Some(lid) = b.lead_id {
        if let Some(lead) = entity::prelude::Lead::find_by_id(lid)
            .filter(entity::lead::Column::TenantId.eq(tenant.tenant_id))
            .filter(entity::lead::Column::ApplicationId.is_null())
            .one(&db)
            .await?
        {
            let mut am: entity::lead::ActiveModel = lead.into();
            am.application_id = Set(Some(saved.id));
            am.status = Set("applied".into());
            am.updated_at = Set(chrono::Utc::now().into());
            am.update(&db).await?;
        }
    }

    let message = if reused_from.is_some() {
        "Welcome back — your recent application was reused and pre-approved for this listing."
    } else {
        "Application received — screening in progress"
    };
    Ok(Json(ApplyResp {
        application_id: saved.id,
        status: saved.status,
        screening_job_id: job_id,
        message: message.into(),
    }))
}
