//! `POST /leads/<id>/convert` — turn a CRM lead into a rental application
//! without leaving the platform. The lead's contact details seed the
//! application; it enters the same intake pipeline as every other door, and the
//! lead is marked `applied` and linked to the new application.

use super::dto::{ConvertLeadReq, ConvertLeadResp, LeadDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::applications::dto::ApplicationResp;
use crate::routes::applications::{intake, IntakeInput};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Lead;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /leads/<id>/convert` — convert a lead into an application.
#[rocket_okapi::openapi(tag = "Leads")]
#[post("/leads/<id>/convert", data = "<body>")]
pub async fn convert_lead(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ConvertLeadReq>,
) -> ApiResult<Json<ConvertLeadResp>> {
    user.require(Permission::ApplicationWrite)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let lead = Lead::find_by_id(lid)
        .filter(entity::lead::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lead not found".into()))?;
    if let Some(app_id) = lead.application_id {
        return Err(ApiError::Conflict(format!(
            "lead already converted to application {app_id}"
        )));
    }

    // Seed the application from the lead and run it through the shared intake
    // pipeline (screening job + staff fan-out + applicant email).
    let (app, _job) = intake(
        &db,
        scope.tenant_id,
        IntakeInput {
            listing_id: b.listing_id,
            applicant_name: lead.name.clone(),
            email: lead.email.clone(),
            phone: lead.phone.clone().unwrap_or_default(),
            annual_income_cents: b.annual_income_cents.unwrap_or(0),
            credit_score: b.credit_score,
            move_in: b.move_in.unwrap_or_default(),
            has_pet: b.has_pet.unwrap_or(false),
            pet_details: b.pet_details,
            is_military: b.is_military.unwrap_or(false),
            // Conversion is staff-driven, like back-office intake: the signed
            // authorization is presumed collected offline unless negated.
            screening_consent: b.screening_consent.unwrap_or(true),
        },
        "crm_lead",
        None,
        Some(user.user_id),
        None,
    )
    .await?;

    let now = Utc::now();
    let mut am: entity::lead::ActiveModel = lead.into();
    am.status = Set("applied".into());
    am.application_id = Set(Some(app.id));
    am.updated_at = Set(now.into());
    let lead = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEAD_CONVERT,
        Some("lead"),
        Some(lead.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "application_id": app.id })),
    )
    .await;

    Ok(Json(ConvertLeadResp {
        lead: LeadDto::from(lead),
        application: ApplicationResp::from(app),
    }))
}
