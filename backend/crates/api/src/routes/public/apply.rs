use super::dto::{ApplyReq, ApplyResp};
use crate::error::ApiResult;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /public/applications` — submit a rental application.
///
/// Persists the application and enqueues a background-screening job that the
/// Tokio scheduler advances asynchronously (submit → await callback → completed).
#[rocket_okapi::openapi(tag = "Public Website")]
#[post("/public/applications", data = "<body>")]
pub async fn apply(
    state: &State<AppState>,
    tenant: PublicTenant,
    body: Json<ApplyReq>,
) -> ApiResult<Json<ApplyResp>> {
    let b = body.into_inner();
    let app_id = Uuid::new_v4();
    let model = entity::application::ActiveModel {
        id: Set(app_id),
        tenant_id: Set(tenant.tenant_id),
        listing_id: Set(b.listing_id),
        applicant_name: Set(b.applicant_name.clone()),
        email: Set(b.email),
        phone: Set(b.phone.unwrap_or_default()),
        annual_income_cents: Set(b.annual_income_cents.unwrap_or(0)),
        credit_score: Set(b.credit_score),
        status: Set("Screening".into()),
        move_in: Set(b.move_in.unwrap_or_default()),
        created_at: Set(Utc::now().into()),
    };
    model.insert(&state.db).await?;

    crate::audit::record(
        &state.db,
        None,
        crate::audit::actions::APPLICATION_SUBMIT,
        Some("application"),
        Some(app_id.to_string()),
        Some(tenant.tenant_id),
        Some(serde_json::json!({ "applicant": b.applicant_name })),
    )
    .await;

    let job_id = scheduler::enqueue(
        &state.db,
        tenant.tenant_id,
        "background_check",
        json!({ "application_id": app_id, "applicant": b.applicant_name }),
        0,
    )
    .await?;

    Ok(Json(ApplyResp {
        application_id: app_id,
        status: "Screening".into(),
        screening_job_id: job_id,
        message: "Application received — screening in progress".into(),
    }))
}
