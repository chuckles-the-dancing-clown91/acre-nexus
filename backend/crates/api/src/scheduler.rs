//! Tokio-driven background job scheduler.
//!
//! This powers the product's "progress automation steps" — durable async work
//! such as awaiting a background-check callback, sending automated emails, or
//! advancing a screening pipeline. Jobs live in the `background_job` table (in
//! the `acre_user` database) so they survive restarts; a single Tokio task polls
//! and advances them.
//!
//! In production each job kind would call out to a real provider (Checkr, an
//! email service, etc.). Here the side effects are simulated but the *state
//! machine and durability* are real.

use chrono::{Duration, Utc};
use entity::prelude::BackgroundJob;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::json;
use std::time::Duration as StdDuration;
use uuid::Uuid;

/// The three domain connections the scheduler makes available to job handlers.
///
/// The `background_job` queue lives in `user`, but advancing a job may require
/// another domain's database — e.g. the property-intelligence enrichment handler
/// writes to `property` — so all three are carried through to each handler.
#[derive(Clone)]
pub struct Pools {
    pub user: DatabaseConnection,
    pub property: DatabaseConnection,
    pub client: DatabaseConnection,
}

/// Default retry budget for jobs enqueued via [`enqueue`].
pub const DEFAULT_MAX_ATTEMPTS: i32 = 5;

/// Enqueue a new background job with the default retry budget. Returns the id.
/// `db` must be the `acre_user` connection (where `background_job` lives).
pub async fn enqueue(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    kind: &str,
    payload: serde_json::Value,
    delay_secs: i64,
) -> Result<Uuid, sea_orm::DbErr> {
    enqueue_with_retries(
        db,
        tenant_id,
        kind,
        payload,
        delay_secs,
        DEFAULT_MAX_ATTEMPTS,
    )
    .await
}

/// Enqueue a new background job with an explicit `max_attempts` retry budget.
pub async fn enqueue_with_retries(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    kind: &str,
    payload: serde_json::Value,
    delay_secs: i64,
    max_attempts: i32,
) -> Result<Uuid, sea_orm::DbErr> {
    let now = Utc::now();
    let job = entity::background_job::ActiveModel {
        id: Set(Uuid::now_v7()),
        tenant_id: Set(tenant_id),
        kind: Set(kind.to_string()),
        status: Set("pending".into()),
        payload: Set(payload),
        result: Set(None),
        run_at: Set((now + Duration::seconds(delay_secs)).into()),
        attempts: Set(0),
        max_attempts: Set(max_attempts.max(1)),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = job.insert(db).await?;
    Ok(saved.id)
}

/// Spawn the scheduler loop on the Tokio runtime. Non-blocking.
pub fn spawn(pools: Pools) {
    tokio::spawn(async move {
        tracing::info!("background scheduler started");
        let mut tick = tokio::time::interval(StdDuration::from_secs(3));
        loop {
            tick.tick().await;
            if let Err(e) = run_due_jobs(&pools).await {
                tracing::error!("scheduler tick failed: {e}");
            }
        }
    });
}

async fn run_due_jobs(pools: &Pools) -> Result<(), sea_orm::DbErr> {
    let now = Utc::now();
    let due = BackgroundJob::find()
        .filter(entity::background_job::Column::RunAt.lte(now))
        .filter(entity::background_job::Column::Status.is_in([
            "pending",
            "running",
            "awaiting_callback",
        ]))
        .order_by_asc(entity::background_job::Column::RunAt)
        .limit(25)
        .all(&pools.user)
        .await?;

    for job in due {
        advance(pools, job).await?;
    }
    Ok(())
}

/// Advance a single job by one state transition.
///
/// Dispatch is **pluggable**: the job's `kind` is routed to the owning module
/// (see [`crate::modules`]). If the owning tenant has disabled that module, the
/// job is parked (`run_at` pushed out, no attempt consumed) rather than
/// processed — re-enabling the module resumes it. Jobs with no owning module
/// fall back to "completed".
async fn advance(
    pools: &Pools,
    job: entity::background_job::Model,
) -> Result<(), sea_orm::DbErr> {
    // background_job lives in the user database; the scheduler runs unclamped
    // (no app.tenant_id) so it can see and advance jobs across all tenants.
    let db = &pools.user;
    let now = Utc::now();
    let mut am: entity::background_job::ActiveModel = job.clone().into();
    am.attempts = Set(job.attempts + 1);
    am.updated_at = Set(now.into());

    match crate::modules::module_for_job_kind(&job.kind) {
        Some(module) => {
            let manifest = module.manifest();
            // Respect per-tenant module enablement for background work too.
            if !crate::modules::is_enabled(db, job.tenant_id, manifest.key).await {
                tracing::debug!(job = %job.id, module = manifest.key, "module disabled; parking job");
                am.run_at = Set((now + Duration::seconds(30)).into());
                am.attempts = Set(job.attempts); // parking doesn't consume an attempt
                am.update(db).await?;
                return Ok(());
            }

            let ctx = crate::modules::JobContext {
                user_db: &pools.user,
                property_db: &pools.property,
                client_db: &pools.client,
                job: &job,
            };
            match module.handle_job(&ctx).await {
                Some(outcome) => {
                    am.status = Set(outcome.status.clone());
                    if let Some(run_at) = outcome.run_at {
                        am.run_at = Set(run_at.into());
                    }
                    if let Some(result) = outcome.result {
                        am.result = Set(Some(result));
                    }
                    if let Some(err) = &outcome.error {
                        am.last_error = Set(Some(err.clone()));
                    }
                    // Enforce the retry budget: a transient retry whose budget is
                    // now exhausted becomes a terminal failure.
                    if outcome.retry && job.attempts + 1 >= job.max_attempts {
                        am.status = Set("failed".into());
                        am.run_at = Set(now.into());
                        am.result = Set(Some(json!({
                            "error": outcome.error.clone().unwrap_or_default(),
                            "attempts": job.attempts + 1,
                            "exhausted": true,
                        })));
                        tracing::warn!(job = %job.id, module = manifest.key, "job failed: retry budget exhausted");
                    } else {
                        tracing::info!(job = %job.id, module = manifest.key, status = %outcome.status, retry = outcome.retry, "job advanced");
                    }
                }
                None => am.status = Set("completed".into()),
            }
        }
        None => {
            tracing::debug!(job = %job.id, kind = %job.kind, "no module owns job kind; completing");
            am.status = Set("completed".into());
            am.result = Set(Some(json!({ "resolved": true })));
        }
    }

    am.update(db).await?;
    Ok(())
}
