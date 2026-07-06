//! Landlord/PM application management (tenant-scoped, RBAC-gated) plus the
//! shared **intake** and **transition** machinery every application door and
//! status change goes through.

pub mod convert;
pub mod create;
pub mod dto;
pub mod list;
pub mod portal;
pub mod reuse;
pub mod screening;
pub mod update_status;
pub mod workflow;

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// Enqueue one applicant-facing email about an application. The
/// `(template, application, trigger)` triple is the notification engine's
/// idempotency key, so re-running a transition (or a retried job) can't
/// double-send.
async fn enqueue_applicant_email(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    app: &entity::application::Model,
    template: &str,
    trigger: &str,
) -> Result<Uuid, sea_orm::DbErr> {
    crate::scheduler::enqueue(
        db,
        tenant_id,
        "auto_email",
        json!({
            "template": template,
            "to": app.email,
            "owner_type": "application",
            "owner_id": app.id,
            "trigger": trigger,
            "vars": { "applicant": app.applicant_name },
        }),
        0,
    )
    .await
}

/// Apply a validated status transition to an application: checks the
/// [`crate::app_workflow`] state machine, updates `status`, records an immutable
/// `application_event`, audits, and fires the applicant-facing side-effects
/// (approval + decline emails). Shared by the `PATCH /applications/<id>` and
/// `POST /applications/<id>/advance` handlers and the screening pipeline
/// (`actor = None` for automated transitions).
pub(crate) async fn apply_transition(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Option<Uuid>,
    app: entity::application::Model,
    to_status: &str,
    note: Option<String>,
) -> ApiResult<entity::application::Model> {
    let from = app.status.clone();
    if !crate::app_workflow::is_known_stage(to_status) {
        return Err(ApiError::BadRequest(format!(
            "unknown application status: {to_status}"
        )));
    }
    if !crate::app_workflow::is_valid_transition(&from, to_status) {
        return Err(ApiError::BadRequest(format!(
            "cannot move an application from '{from}' to '{to_status}'"
        )));
    }

    let mut am: entity::application::ActiveModel = app.into();
    am.status = Set(to_status.to_string());
    let mut saved = am.update(db).await?;

    entity::application_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        application_id: Set(saved.id),
        from_status: Set(Some(from.clone())),
        to_status: Set(to_status.to_string()),
        note: Set(note.clone()),
        actor_user_id: Set(actor),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        actor,
        crate::audit::actions::APPLICATION_ADVANCE,
        Some("application"),
        Some(saved.id.to_string()),
        Some(tenant_id),
        Some(json!({ "from": from, "to": to_status, "note": note })),
    )
    .await;

    // Applicant-facing side-effects. The owner/trigger fields give the
    // notification engine its idempotency key, so re-running a transition (or
    // a retried job) can't double-send.
    match to_status {
        "Approved" => {
            let _ =
                enqueue_applicant_email(db, tenant_id, &saved, "application_approved", "approved")
                    .await;
        }
        "Declined" => {
            let _ =
                enqueue_applicant_email(db, tenant_id, &saved, "application_declined", "declined")
                    .await;
            // FCRA §615(a): when the decline follows a report with adverse
            // information (and the workspace setting is on), send + file the
            // adverse-action notice automatically — it stamps the application,
            // so return the fresh row.
            crate::screening::maybe_auto_adverse_action(db, tenant_id, actor, &saved).await;
            if let Some(fresh) = entity::prelude::Application::find_by_id(saved.id)
                .filter(entity::application::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
            {
                saved = fresh;
            }
        }
        _ => {}
    }

    Ok(saved)
}

// ---------------------------------------------------------------------------
// Intake — the one path every application door goes through
// ---------------------------------------------------------------------------

/// The normalized applicant data an intake door collects.
pub(crate) struct IntakeInput {
    pub listing_id: Option<Uuid>,
    pub applicant_name: String,
    pub email: String,
    pub phone: String,
    pub annual_income_cents: i64,
    pub credit_score: Option<i32>,
    pub move_in: String,
    pub has_pet: bool,
    pub pet_details: Option<String>,
    pub is_military: bool,
    /// The applicant authorized a consumer report (FCRA §604(b)). Required by
    /// every door that screens; a reused approval carries the prior consent.
    pub screening_consent: bool,
}

/// Create an application and run the standard submission side-effects,
/// identically for every door (public website, renter portal, back office):
///
/// 1. persist the row (status `Screening`, or `Approved` when a reusable prior
///    approval is carried forward),
/// 2. audit the submission,
/// 3. fan the event out to staff holding `application:read` (in-app + push +
///    chat), excluding the acting staff member on back-office intake,
/// 4. email the applicant — "application received", or "approved" when reused,
/// 5. enqueue the background-screening job (skipped when pre-approved).
///
/// Returns the saved application plus the id of the job it kicked off.
pub(crate) async fn intake(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    input: IntakeInput,
    source: &str,
    applicant_user_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    reused_from: Option<&entity::application::Model>,
) -> ApiResult<(entity::application::Model, Uuid)> {
    let name = input.applicant_name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("applicant name is required".into()));
    }
    let email = input.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(ApiError::BadRequest(format!(
            "invalid applicant email '{email}'"
        )));
    }
    // Screening runs on every fresh application, and a consumer report may
    // only be ordered with the applicant's written authorization (FCRA
    // §604(b)) — no consent, no application. A reused approval doesn't
    // re-screen, so it rides the prior application's consent instead.
    if reused_from.is_none() && !input.screening_consent {
        return Err(ApiError::BadRequest(
            "screening consent is required to submit an application".into(),
        ));
    }
    // A listing reference must be real (and this tenant's) — a typo'd or
    // cross-tenant id would silently detach the application from its home.
    if let Some(listing_id) = input.listing_id {
        entity::prelude::Listing::find_by_id(listing_id)
            .filter(entity::listing::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::BadRequest("listing not found".into()))?;
    }

    let app_id = Uuid::new_v4();
    let status = if reused_from.is_some() {
        "Approved"
    } else {
        "Screening"
    };
    // Prefer a carried-forward credit score when the applicant didn't supply one.
    let credit_score = input
        .credit_score
        .or_else(|| reused_from.and_then(|r| r.credit_score));

    let saved = entity::application::ActiveModel {
        id: Set(app_id),
        tenant_id: Set(tenant_id),
        listing_id: Set(input.listing_id),
        applicant_name: Set(name.clone()),
        email: Set(email.clone()),
        phone: Set(input.phone.trim().to_string()),
        annual_income_cents: Set(input.annual_income_cents),
        credit_score: Set(credit_score),
        status: Set(status.into()),
        move_in: Set(input.move_in),
        has_pet: Set(input.has_pet),
        pet_details: Set(input.pet_details),
        is_military: Set(input.is_military),
        source: Set(source.to_string()),
        applicant_user_id: Set(applicant_user_id),
        // A reused approval carries the prior screening outcome forward.
        screening_status: Set(reused_from.and_then(|r| r.screening_status.clone())),
        screened_at: Set(reused_from.and_then(|r| r.screened_at)),
        created_at: Set(Utc::now().into()),
        screening_consent_at: Set(if input.screening_consent {
            Some(Utc::now().into())
        } else {
            reused_from.and_then(|r| r.screening_consent_at)
        }),
        adverse_action_at: Set(None),
        adverse_action_document_id: Set(None),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        actor_user_id,
        crate::audit::actions::APPLICATION_SUBMIT,
        Some("application"),
        Some(app_id.to_string()),
        Some(tenant_id),
        Some(json!({ "applicant": name, "source": source })),
    )
    .await;

    // Integrated notifications: every staff member who can read applications
    // gets an in-app inbox entry + a web push, and the tenant's chat channel
    // (if configured) gets one message.
    crate::notify::notify_staff(
        db,
        tenant_id,
        "application:read",
        "application_submitted",
        json!({ "applicant": name }),
        Some(("application", app_id)),
        "submitted",
        actor_user_id,
    )
    .await;

    // The applicant always hears back immediately, and pre-approved (reused)
    // applicants skip re-screening while everyone else enters the
    // background-check pipeline. The returned job id is the pipeline's next
    // step: the screening job, or the approval email when there is nothing to
    // screen.
    let job_id = if reused_from.is_some() {
        // Pre-approved: skip "received", go straight to the good news.
        enqueue_applicant_email(
            db,
            tenant_id,
            &saved,
            "application_approved",
            "pre_approved",
        )
        .await?
    } else {
        let _ = enqueue_applicant_email(db, tenant_id, &saved, "application_received", "submitted")
            .await;
        crate::scheduler::enqueue(
            db,
            tenant_id,
            "background_check",
            json!({ "application_id": app_id, "applicant": name }),
            0,
        )
        .await?
    };

    Ok((saved, job_id))
}
