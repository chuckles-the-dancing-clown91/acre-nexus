//! `POST /applications` — **back-office intake**: staff take an application on
//! an applicant's behalf (walk-in, phone, email) and it enters the exact same
//! pipeline as the website funnel: screening job, staff fan-out, and the
//! applicant's "application received" email.

use super::dto::{ApplicationResp, CreateApplicationReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `POST /applications` — staff intake of a rental application.
#[rocket_okapi::openapi(tag = "Applications")]
#[post("/applications", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateApplicationReq>,
) -> ApiResult<Json<ApplicationResp>> {
    user.require(Permission::ApplicationWrite)?;
    let b = body.into_inner();

    // A referenced listing must be this workspace's.
    if let Some(lid) = b.listing_id {
        Listing::find_by_id(lid)
            .filter(entity::listing::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("listing not found".into()))?;
    }

    let (saved, _job) = super::intake(
        &db,
        scope.tenant_id,
        super::IntakeInput {
            listing_id: b.listing_id,
            applicant_name: b.applicant_name,
            email: b.email,
            phone: b.phone.unwrap_or_default(),
            annual_income_cents: b.annual_income_cents.unwrap_or(0),
            credit_score: b.credit_score,
            move_in: b.move_in.unwrap_or_default(),
            has_pet: b.has_pet.unwrap_or(false),
            pet_details: b.pet_details,
            is_military: b.is_military.unwrap_or(false),
            // Staff intake implies the signed authorization was collected
            // outside the system, unless explicitly negated.
            screening_consent: b.screening_consent.unwrap_or(true),
        },
        "back_office",
        None,
        Some(user.user_id),
        None,
    )
    .await?;

    Ok(Json(ApplicationResp::from(saved)))
}
