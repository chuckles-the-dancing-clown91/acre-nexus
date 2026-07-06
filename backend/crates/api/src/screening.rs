//! **FCRA screening orchestration** (roadmap Phase 4, epic #8).
//!
//! The apply funnel's `background_check` job rides this module: it orders a
//! [`entity::screening_report`] through the Checkr provider (deterministic
//! simulation by default, live via `LIVE_PROVIDERS=checkr`), waits for the
//! result (the tenant's callback-delay setting in simulation, the provider
//! webhook live), evaluates the workspace's screening policy against the
//! report, and lands the verdict on the application through the same
//! `application.screened` slot Phase 2 established — auto-approve included.
//!
//! FCRA discipline lives here too: a report is only ordered with the
//! applicant's **consent** stamped at intake, and declining an applicant
//! whose report carried adverse information sends (and files) an
//! **adverse-action notice** naming the consumer-reporting agency and the
//! applicant's rights.

use crate::error::{ApiError, ApiResult};
use crate::modules::JobOutcome;
use crate::providers::screening::{CheckrProvider, ScreeningRequest, ScreeningResponse};
use crate::providers::ProviderCtx;
use crate::storage::ObjectStore;
use chrono::Utc;
use entity::prelude::{Application, ScreeningReport};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Report lifecycle
// ---------------------------------------------------------------------------

/// Find or create the application's screening report (one per application;
/// retried jobs reuse the same row).
pub async fn ensure_report(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    app: &entity::application::Model,
) -> Result<entity::screening_report::Model, sea_orm::DbErr> {
    if let Some(existing) = ScreeningReport::find()
        .filter(entity::screening_report::Column::TenantId.eq(tenant_id))
        .filter(entity::screening_report::Column::ApplicationId.eq(app.id))
        .one(db)
        .await?
    {
        return Ok(existing);
    }
    let now = Utc::now();
    entity::screening_report::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        application_id: Set(app.id),
        provider: Set("checkr".into()),
        external_id: Set(None),
        status: Set("pending".into()),
        include_credit: Set(true),
        include_criminal: Set(true),
        include_eviction: Set(true),
        consent_at: Set(app.screening_consent_at),
        credit_score: Set(None),
        criminal_records: Set(None),
        eviction_records: Set(None),
        recommendation: Set(None),
        result: Set(None),
        reasons: Set(None),
        completed_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
}

/// Advance one `background_check` / `screening` job. Two phases, exactly like
/// the Phase 2 pipeline, but the verdict now comes from a real report:
///
/// * `pending` — order the report (live mode calls the provider now so the
///   webhook has something to complete; simulation defers to the callback
///   phase), then wait out the tenant's callback delay;
/// * `awaiting_callback` — obtain results (simulate now, or read what the
///   webhook landed), evaluate policy, and land the verdict.
pub async fn handle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(app_id) = job
        .payload
        .get("application_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        // Legacy/manual jobs without an application reference: nothing to do.
        return JobOutcome::completed(json!({ "resolved": true, "reason": "no application" }));
    };
    let tenant_id = job.tenant_id;
    let app = match Application::find_by_id(app_id)
        .filter(entity::application::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    {
        Ok(Some(a)) => a,
        Ok(None) => return JobOutcome::completed(json!({ "resolved": true, "reason": "gone" })),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };

    match job.status.as_str() {
        "pending" => {
            let report = match ensure_report(db, tenant_id, &app).await {
                Ok(r) => r,
                Err(e) => {
                    return JobOutcome::retry(
                        crate::providers::backoff(job.attempts),
                        format!("report create failed: {e}"),
                    )
                }
            };

            crate::audit::record(
                db,
                None,
                crate::audit::actions::SCREENING_ORDERED,
                Some("screening_report"),
                Some(report.id.to_string()),
                Some(tenant_id),
                Some(json!({
                    "application_id": app.id,
                    "provider": report.provider,
                    "consent_at": report.consent_at.map(|t| t.to_rfc3339()),
                })),
            )
            .await;

            // Live mode orders now so the webhook can complete it; the
            // simulator produces its report at callback time.
            if crate::providers::is_live("checkr") && report.external_id.is_none() {
                let ctx = ProviderCtx::new(db, tenant_id);
                let req = request_for(&report, &app);
                match crate::providers::run(&CheckrProvider, &ctx, job, &req).await {
                    Ok(resp) => {
                        let mut am: entity::screening_report::ActiveModel = report.into();
                        am.external_id = Set(Some(resp.external_id));
                        am.status = Set("in_progress".into());
                        am.updated_at = Set(Utc::now().into());
                        if let Err(e) = am.update(db).await {
                            return JobOutcome::retry(
                                crate::providers::backoff(job.attempts),
                                format!("db error: {e}"),
                            );
                        }
                    }
                    Err(outcome) => return outcome,
                }
            }

            let delay = crate::settings::get_i64(
                db,
                tenant_id,
                crate::settings::SCREENING_CALLBACK_DELAY_SECS,
            )
            .await
            .max(0);
            JobOutcome::reschedule("awaiting_callback", delay)
        }
        "awaiting_callback" => {
            let report = match ensure_report(db, tenant_id, &app).await {
                Ok(r) => r,
                Err(e) => {
                    return JobOutcome::retry(
                        crate::providers::backoff(job.attempts),
                        format!("report load failed: {e}"),
                    )
                }
            };

            // Already landed (webhook beat us, or a retry): done.
            if report.result.is_some() {
                return JobOutcome::completed(json!({
                    "result": report.result,
                    "already_landed": true,
                }));
            }

            let report = if crate::providers::is_live("checkr") {
                // The webhook owns completion in live mode; keep checking in.
                if report.status != "complete" {
                    return JobOutcome::reschedule("awaiting_callback", 300);
                }
                report
            } else {
                // Simulated bureau answers now, deterministically.
                let ctx = ProviderCtx::new(db, tenant_id);
                let req = request_for(&report, &app);
                let resp = match crate::providers::run(&CheckrProvider, &ctx, job, &req).await {
                    Ok(resp) => resp,
                    Err(outcome) => return outcome,
                };
                match write_results(db, report, &resp).await {
                    Ok(r) => r,
                    Err(e) => {
                        return JobOutcome::retry(
                            crate::providers::backoff(job.attempts),
                            format!("db error: {e}"),
                        )
                    }
                }
            };

            match land_outcome(db, tenant_id, app, report).await {
                Ok(result) => JobOutcome::completed(json!({
                    "result": result,
                    "completed_at": Utc::now().to_rfc3339(),
                })),
                // Landing the outcome is the whole point of the job — retry.
                Err(e) => JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("failed to land screening outcome: {e}"),
                ),
            }
        }
        _ => JobOutcome::completed(json!({ "resolved": true })),
    }
}

fn request_for(
    report: &entity::screening_report::Model,
    app: &entity::application::Model,
) -> ScreeningRequest {
    ScreeningRequest {
        reference: report.id,
        candidate_name: app.applicant_name.clone(),
        email: app.email.clone(),
        stated_credit_score: app.credit_score,
        include_credit: report.include_credit,
        include_criminal: report.include_criminal,
        include_eviction: report.include_eviction,
    }
}

/// Persist provider results onto the report (status `complete`, verdict still
/// pending until [`land_outcome`]).
async fn write_results(
    db: &impl ConnectionTrait,
    report: entity::screening_report::Model,
    resp: &ScreeningResponse,
) -> Result<entity::screening_report::Model, sea_orm::DbErr> {
    let mut am: entity::screening_report::ActiveModel = report.into();
    am.external_id = Set(Some(resp.external_id.clone()));
    am.status = Set("complete".into());
    am.credit_score = Set(resp.credit_score);
    am.criminal_records = Set(resp.criminal_records);
    am.eviction_records = Set(resp.eviction_records);
    am.recommendation = Set(resp.recommendation.clone());
    am.updated_at = Set(Utc::now().into());
    am.update(db).await
}

// ---------------------------------------------------------------------------
// Policy + landing
// ---------------------------------------------------------------------------

/// Evaluate the workspace's screening policy against a completed report.
/// Pure — the whole verdict surface is unit-testable.
pub fn evaluate_policy(
    min_credit_score: i64,
    min_income_rent_ratio: i64,
    monthly_rent_cents: i64,
    annual_income_cents: i64,
    credit_score: Option<i32>,
    criminal_records: Option<i32>,
    eviction_records: Option<i32>,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if min_credit_score > 0 {
        if let Some(score) = credit_score {
            if i64::from(score) < min_credit_score {
                reasons.push(format!(
                    "credit score {score} below minimum {min_credit_score}"
                ));
            }
        }
    }
    if min_income_rent_ratio > 0
        && monthly_rent_cents > 0
        && annual_income_cents < min_income_rent_ratio * monthly_rent_cents * 12
    {
        reasons.push(format!(
            "monthly income ${:.0} below {min_income_rent_ratio}x rent ${:.0}",
            annual_income_cents as f64 / 12.0 / 100.0,
            monthly_rent_cents as f64 / 100.0,
        ));
    }
    if criminal_records.unwrap_or(0) > 0 {
        reasons.push(format!(
            "{} criminal record(s) reported",
            criminal_records.unwrap_or(0)
        ));
    }
    if eviction_records.unwrap_or(0) > 0 {
        reasons.push(format!(
            "{} prior eviction(s) reported",
            eviction_records.unwrap_or(0)
        ));
    }
    reasons
}

/// Land a completed report on its application: final verdict, audit, and the
/// Phase 2 side-effects (auto-approve or staff notification). Idempotent.
pub async fn land_outcome(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    app: entity::application::Model,
    report: entity::screening_report::Model,
) -> anyhow::Result<String> {
    // Policy inputs.
    let min_score =
        crate::settings::get_i64(db, tenant_id, crate::settings::SCREENING_MIN_CREDIT_SCORE).await;
    let min_ratio = crate::settings::get_i64(
        db,
        tenant_id,
        crate::settings::SCREENING_MIN_INCOME_RENT_RATIO,
    )
    .await;
    let rent_cents = match app.listing_id {
        Some(listing_id) => entity::prelude::Listing::find_by_id(listing_id)
            .filter(entity::listing::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .ok()
            .flatten()
            .map(|l| l.rent_cents)
            .unwrap_or(0),
        None => 0,
    };

    let reasons = evaluate_policy(
        min_score,
        min_ratio,
        rent_cents,
        app.annual_income_cents,
        report.credit_score,
        report.criminal_records,
        report.eviction_records,
    );
    let result = if reasons.is_empty() {
        "cleared"
    } else {
        "failed"
    };
    let now = Utc::now();

    // Verdict onto the report…
    let report_id = report.id;
    let mut ram: entity::screening_report::ActiveModel = report.into();
    ram.result = Set(Some(result.to_string()));
    ram.reasons = Set(Some(json!(reasons)));
    ram.status = Set("complete".into());
    ram.completed_at = Set(Some(now.into()));
    ram.updated_at = Set(now.into());
    ram.update(db).await?;

    // …and onto the application (idempotent re-write on retries).
    let mut am: entity::application::ActiveModel = app.clone().into();
    am.screening_status = Set(Some(result.to_string()));
    am.screened_at = Set(Some(now.into()));
    let app = am.update(db).await?;

    crate::audit::record(
        db,
        None,
        crate::audit::actions::SCREENING_COMPLETED,
        Some("screening_report"),
        Some(report_id.to_string()),
        Some(tenant_id),
        Some(json!({ "application_id": app.id, "result": result, "reasons": reasons })),
    )
    .await;
    // The application-level event Phase 2 established — same slot, real data.
    crate::audit::record(
        db,
        None,
        crate::audit::actions::APPLICATION_SCREENED,
        Some("application"),
        Some(app.id.to_string()),
        Some(tenant_id),
        Some(json!({ "result": result, "reasons": reasons })),
    )
    .await;

    // The application may have been decided while screening ran.
    if app.status != "Screening" {
        return Ok(result.to_string());
    }

    let auto_approve =
        crate::settings::get_bool(db, tenant_id, crate::settings::APPLICATION_AUTO_APPROVE).await;
    if auto_approve && result == "cleared" {
        crate::routes::applications::apply_transition(
            db,
            tenant_id,
            None,
            app,
            "Approved",
            Some("Auto-approved: screening cleared".into()),
        )
        .await
        .map_err(|e| anyhow::anyhow!("auto-approve transition failed: {e}"))?;
    } else {
        crate::notify::notify_staff(
            db,
            tenant_id,
            "application:read",
            "application_screened",
            json!({ "applicant": app.applicant_name, "result": result }),
            Some(("application", app.id)),
            "screened",
            None,
        )
        .await;
    }
    Ok(result.to_string())
}

// ---------------------------------------------------------------------------
// Webhook (live Checkr)
// ---------------------------------------------------------------------------

/// Handle one verified `webhook_event` for the screening provider. `None`
/// when the event isn't ours.
pub async fn handle_webhook_event(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> Option<JobOutcome> {
    let provider = job.payload.get("provider").and_then(|v| v.as_str())?;
    if provider != "checkr" {
        return None;
    }
    let event = job.payload.get("event").cloned().unwrap_or(json!({}));
    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let object = event.pointer("/data/object").cloned().unwrap_or(json!({}));

    if event_type != "report.completed" {
        return Some(JobOutcome::completed(json!({
            "provider": "checkr",
            "event": event_type,
            "ignored": true,
        })));
    }

    let external_id = object.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let reference = object
        .pointer("/metadata/reference")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    // Find the report by provider id, falling back to our reference.
    let mut report = None;
    if !external_id.is_empty() {
        report = ScreeningReport::find()
            .filter(entity::screening_report::Column::TenantId.eq(job.tenant_id))
            .filter(entity::screening_report::Column::ExternalId.eq(external_id))
            .one(db)
            .await
            .ok()
            .flatten();
    }
    if report.is_none() {
        if let Some(id) = reference {
            report = ScreeningReport::find_by_id(id)
                .filter(entity::screening_report::Column::TenantId.eq(job.tenant_id))
                .one(db)
                .await
                .ok()
                .flatten();
        }
    }
    let Some(report) = report else {
        return Some(JobOutcome::failed(format!(
            "checkr report.completed matched no screening report (id {external_id})"
        )));
    };

    let resp = ScreeningResponse {
        external_id: if external_id.is_empty() {
            report.external_id.clone().unwrap_or_default()
        } else {
            external_id.to_string()
        },
        status: "complete".into(),
        credit_score: object
            .get("credit_score")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        criminal_records: object
            .get("criminal_records")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        eviction_records: object
            .get("eviction_records")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        recommendation: object
            .get("assessment")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    };
    let report = match write_results(db, report, &resp).await {
        Ok(r) => r,
        Err(e) => return Some(JobOutcome::failed(format!("db error: {e}"))),
    };

    let app = Application::find_by_id(report.application_id)
        .filter(entity::application::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    if let Some(app) = app {
        if let Err(e) = land_outcome(db, job.tenant_id, app, report).await {
            return Some(JobOutcome::failed(format!(
                "failed to land webhook outcome: {e}"
            )));
        }
    }
    Some(JobOutcome::completed(json!({
        "provider": "checkr",
        "event": event_type,
        "landed": true,
    })))
}

// ---------------------------------------------------------------------------
// Adverse action (FCRA §615(a))
// ---------------------------------------------------------------------------

/// Whether the report carries adverse information a decline notice must cover.
pub fn has_adverse_information(report: &entity::screening_report::Model) -> bool {
    report.result.as_deref() == Some("failed")
        || report.criminal_records.unwrap_or(0) > 0
        || report.eviction_records.unwrap_or(0) > 0
}

/// Send (and file) the adverse-action notice for a declined application:
/// generate the notice from the report + CRA settings, store the PDF against
/// the application, stamp the application, audit, and email the applicant.
pub async fn send_adverse_action(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Option<Uuid>,
    app: entity::application::Model,
) -> ApiResult<entity::application::Model> {
    if app.adverse_action_at.is_some() {
        return Err(ApiError::Conflict(
            "an adverse-action notice was already sent for this application".into(),
        ));
    }
    let report = ScreeningReport::find()
        .filter(entity::screening_report::Column::TenantId.eq(tenant_id))
        .filter(entity::screening_report::Column::ApplicationId.eq(app.id))
        .one(db)
        .await?
        .ok_or_else(|| {
            ApiError::BadRequest("no screening report exists for this application".into())
        })?;
    if report.completed_at.is_none() {
        return Err(ApiError::BadRequest(
            "the screening report has not completed yet".into(),
        ));
    }

    let company = crate::esign::tenant_slug(db, tenant_id).await; // slug fallback
    let company_name = entity::prelude::Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|t| t.company_name)
        .unwrap_or(company);
    let cra_name =
        crate::settings::get_string(db, tenant_id, crate::settings::SCREENING_CRA_NAME).await;
    let cra_contact =
        crate::settings::get_string(db, tenant_id, crate::settings::SCREENING_CRA_CONTACT).await;
    let reasons: Vec<String> = report
        .reasons
        .as_ref()
        .and_then(|v| v.as_array().cloned())
        .map(|a| {
            a.into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let today = Utc::now().date_naive().to_string();
    let text = adverse_action_text(
        &company_name,
        &app.applicant_name,
        &today,
        &reasons,
        &cra_name,
        &cra_contact,
    );

    // File the notice in the document service (best-effort storage must not
    // block the legally-required send — but the document *is* the record, so
    // a storage failure here is a real error).
    let bytes = crate::pdf::text_to_pdf(&text);
    let doc_id = Uuid::new_v4();
    let storage_key = format!("{tenant_id}/{doc_id}");
    let store = ObjectStore::from_env().map_err(ApiError::Internal)?;
    store
        .put_bytes(&storage_key, &bytes)
        .await
        .map_err(ApiError::Internal)?;
    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(tenant_id),
        owner_type: Set("application".into()),
        owner_id: Set(app.id),
        filename: Set("adverse-action-notice.pdf".into()),
        mime_type: Set("application/pdf".into()),
        size_bytes: Set(bytes.len() as i64),
        checksum: Set(Some(crate::storage::sha256_hex(&bytes))),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(storage_key),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(actor),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    let mut am: entity::application::ActiveModel = app.clone().into();
    am.adverse_action_at = Set(Some(now.into()));
    am.adverse_action_document_id = Set(Some(doc_id));
    let saved = am.update(db).await?;

    entity::application_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        application_id: Set(saved.id),
        from_status: Set(Some(saved.status.clone())),
        to_status: Set(saved.status.clone()),
        note: Set(Some("Adverse-action notice sent (FCRA §615(a))".into())),
        actor_user_id: Set(actor),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        actor,
        crate::audit::actions::ADVERSE_ACTION,
        Some("application"),
        Some(saved.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "document_id": doc_id,
            "report_id": report.id,
            "reasons": reasons,
        })),
    )
    .await;

    // The applicant-facing notice email.
    let payload = json!({
        "template": "adverse_action",
        "to": saved.email,
        "owner_type": "application",
        "owner_id": saved.id,
        "trigger": "adverse_action",
        "vars": {
            "applicant": saved.applicant_name,
            "cra_name": cra_name,
            "cra_contact": cra_contact,
        },
    });
    if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_email", payload, 0).await {
        tracing::error!("failed to enqueue adverse-action email: {e}");
    }

    Ok(saved)
}

/// Auto-send on decline when the workspace setting is on and the report
/// carried adverse information. Best-effort: a failure logs (staff can send
/// manually from the console) rather than blocking the decline.
pub async fn maybe_auto_adverse_action(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Option<Uuid>,
    app: &entity::application::Model,
) {
    if app.adverse_action_at.is_some() {
        return;
    }
    if !crate::settings::get_bool(
        db,
        tenant_id,
        crate::settings::SCREENING_AUTO_ADVERSE_ACTION,
    )
    .await
    {
        return;
    }
    let report = ScreeningReport::find()
        .filter(entity::screening_report::Column::TenantId.eq(tenant_id))
        .filter(entity::screening_report::Column::ApplicationId.eq(app.id))
        .one(db)
        .await
        .ok()
        .flatten();
    let Some(report) = report else { return };
    if report.completed_at.is_none() || !has_adverse_information(&report) {
        return;
    }
    if let Err(e) = send_adverse_action(db, tenant_id, actor, app.clone()).await {
        tracing::error!("auto adverse-action failed for {}: {e}", app.id);
    }
}

/// The rendered adverse-action notice — the FCRA §615(a) essentials: the
/// decision, the CRA that furnished the report (and that it didn't make the
/// decision), and the applicant's rights (free copy within 60 days, dispute).
pub fn adverse_action_text(
    company: &str,
    applicant: &str,
    date: &str,
    reasons: &[String],
    cra_name: &str,
    cra_contact: &str,
) -> String {
    let reason_lines = if reasons.is_empty() {
        "  - Information contained in your consumer report".to_string()
    } else {
        reasons
            .iter()
            .map(|r| format!("  - {r}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "ADVERSE ACTION NOTICE\n\
         ================================\n\n\
         Date:      {date}\n\
         To:        {applicant}\n\
         From:      {company}\n\n\
         This notice is provided under the Fair Credit Reporting Act (FCRA).\n\n\
         Your rental application has been declined, based in whole or in part\n\
         on information obtained from a consumer report. Factors included:\n\n\
         {reason_lines}\n\n\
         The report was furnished by:\n\n\
         {cra_name}\n\
         {cra_contact}\n\n\
         The consumer reporting agency did not make this decision and cannot\n\
         explain why it was made.\n\n\
         YOUR RIGHTS UNDER THE FCRA\n\
         --------------------------------\n\
         - You may obtain a free copy of your consumer report from the agency\n\
           named above within 60 days of this notice.\n\
         - You have the right to dispute directly with the agency the accuracy\n\
           or completeness of any information in the report.\n\n\
         — {company}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_clears_when_nothing_trips() {
        let reasons = evaluate_policy(0, 0, 185_000, 9_000_000, Some(700), Some(0), Some(0));
        assert!(reasons.is_empty());
    }

    #[test]
    fn policy_trips_credit_floor_income_ratio_and_records() {
        // Floor 650 vs score 600; 3x rent $1,850 needs $66,600/yr vs $30k;
        // one criminal and one eviction record.
        let reasons = evaluate_policy(650, 3, 185_000, 3_000_000, Some(600), Some(1), Some(1));
        assert_eq!(reasons.len(), 4);
        assert!(reasons[0].contains("credit score 600 below minimum 650"));
        assert!(reasons[1].contains("below 3x rent"));
        assert!(reasons[2].contains("criminal record"));
        assert!(reasons[3].contains("eviction"));
    }

    #[test]
    fn policy_skips_checks_without_data() {
        // No credit score reported → the floor can't fail it; no listing rent
        // → the ratio can't fail it.
        let reasons = evaluate_policy(650, 3, 0, 3_000_000, None, None, None);
        assert!(reasons.is_empty());
    }

    #[test]
    fn adverse_notice_carries_the_fcra_essentials() {
        let text = adverse_action_text(
            "Northwind Property Group",
            "Jordan Avery",
            "2026-07-05",
            &["credit score 600 below minimum 650".into()],
            "Acme Screening Bureau",
            "dispute@acme.example · (800) 555-0100",
        );
        assert!(text.contains("ADVERSE ACTION NOTICE"));
        assert!(text.contains("Jordan Avery"));
        assert!(text.contains("credit score 600 below minimum 650"));
        assert!(text.contains("Acme Screening Bureau"));
        assert!(text.contains("free copy"));
        assert!(text.contains("dispute"));
        assert!(text.contains("did not make this decision"));
    }

    #[test]
    fn adverse_information_detection() {
        let mut report = entity::screening_report::Model {
            id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            application_id: Uuid::from_u128(3),
            provider: "checkr".into(),
            external_id: None,
            status: "complete".into(),
            include_credit: true,
            include_criminal: true,
            include_eviction: true,
            consent_at: None,
            credit_score: Some(720),
            criminal_records: Some(0),
            eviction_records: Some(0),
            recommendation: Some("clear".into()),
            result: Some("cleared".into()),
            reasons: None,
            completed_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        assert!(!has_adverse_information(&report));
        report.result = Some("failed".into());
        assert!(has_adverse_information(&report));
        report.result = Some("cleared".into());
        report.eviction_records = Some(1);
        assert!(has_adverse_information(&report));
    }
}
