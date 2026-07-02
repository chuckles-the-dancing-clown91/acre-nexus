use super::dto::{ApplyReq, ApplyResp};
use crate::error::ApiResult;
use crate::scheduler;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use chrono::Utc;
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /public/applications` — submit a rental application.
///
/// Persists the application and enqueues a background-screening job that the
/// Tokio scheduler advances asynchronously (submit → await callback → completed).
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
    let app_id = Uuid::new_v4();

    // Reuse: if the workspace allows it and this applicant already has a recent
    // *approved* application, carry that screening result forward — the new
    // application is pre-approved for this listing and skips re-screening.
    let reused_from =
        match crate::routes::applications::reuse::reuse_cutoff(&db, tenant.tenant_id).await {
            Some(cutoff) => {
                Application::find()
                    .filter(entity::application::Column::TenantId.eq(tenant.tenant_id))
                    .filter(entity::application::Column::Email.eq(email.clone()))
                    .filter(entity::application::Column::Status.eq("Approved"))
                    .filter(entity::application::Column::CreatedAt.gte(cutoff))
                    .order_by_desc(entity::application::Column::CreatedAt)
                    .one(&db)
                    .await?
            }
            None => None,
        };

    let status = if reused_from.is_some() {
        "Approved"
    } else {
        "Screening"
    };
    // Prefer a carried-forward credit score when the applicant didn't supply one.
    let credit_score = b
        .credit_score
        .or_else(|| reused_from.as_ref().and_then(|r| r.credit_score));

    let model = entity::application::ActiveModel {
        id: Set(app_id),
        tenant_id: Set(tenant.tenant_id),
        listing_id: Set(b.listing_id),
        applicant_name: Set(b.applicant_name.clone()),
        email: Set(email.clone()),
        phone: Set(b.phone.unwrap_or_default()),
        annual_income_cents: Set(b.annual_income_cents.unwrap_or(0)),
        credit_score: Set(credit_score),
        status: Set(status.into()),
        move_in: Set(b.move_in.unwrap_or_default()),
        has_pet: Set(b.has_pet.unwrap_or(false)),
        pet_details: Set(b.pet_details.clone()),
        is_military: Set(b.is_military.unwrap_or(false)),
        created_at: Set(Utc::now().into()),
    };
    model.insert(&db).await?;

    crate::audit::record(
        &db,
        None,
        crate::audit::actions::APPLICATION_SUBMIT,
        Some("application"),
        Some(app_id.to_string()),
        Some(tenant.tenant_id),
        Some(serde_json::json!({ "applicant": b.applicant_name })),
    )
    .await;

    // Pre-approved (reused) applicants skip re-screening; everyone else enters the
    // background-check pipeline. Either way we kick off a job and return its id.
    let (job_id, message) = if reused_from.is_some() {
        let jid = scheduler::enqueue(
            &db,
            tenant.tenant_id,
            "auto_email",
            json!({
                "template": "application_approved",
                "to": email,
                "owner_type": "application",
                "owner_id": app_id,
                "trigger": "pre_approved",
            }),
            0,
        )
        .await?;
        (
            jid,
            "Welcome back — your recent application was reused and pre-approved for this listing.",
        )
    } else {
        let jid = scheduler::enqueue(
            &db,
            tenant.tenant_id,
            "background_check",
            json!({ "application_id": app_id, "applicant": b.applicant_name }),
            0,
        )
        .await?;
        (jid, "Application received — screening in progress")
    };

    Ok(Json(ApplyResp {
        application_id: app_id,
        status: status.into(),
        screening_job_id: job_id,
        message: message.into(),
    }))
}
