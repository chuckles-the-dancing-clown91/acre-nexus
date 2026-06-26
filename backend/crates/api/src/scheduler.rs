//! Tokio-driven background job scheduler.
//!
//! This powers the product's "progress automation steps" — durable async work
//! such as awaiting a background-check callback, sending automated emails, or
//! advancing a screening pipeline. Jobs live in the `background_job` table so they
//! survive restarts; a single Tokio task polls and advances them.
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

/// Enqueue a new background job. Returns the job id.
pub async fn enqueue(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    kind: &str,
    payload: serde_json::Value,
    delay_secs: i64,
) -> Result<Uuid, sea_orm::DbErr> {
    let now = Utc::now();
    let job = entity::background_job::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        kind: Set(kind.to_string()),
        status: Set("pending".into()),
        payload: Set(payload),
        result: Set(None),
        run_at: Set((now + Duration::seconds(delay_secs)).into()),
        attempts: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = job.insert(db).await?;
    Ok(saved.id)
}

/// Spawn the scheduler loop on the Tokio runtime. Non-blocking.
pub fn spawn(db: DatabaseConnection) {
    tokio::spawn(async move {
        tracing::info!("background scheduler started");
        let mut tick = tokio::time::interval(StdDuration::from_secs(3));
        loop {
            tick.tick().await;
            if let Err(e) = run_due_jobs(&db).await {
                tracing::error!("scheduler tick failed: {e}");
            }
        }
    });
}

async fn run_due_jobs(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let now = Utc::now();
    let due = BackgroundJob::find()
        .filter(entity::background_job::Column::RunAt.lte(now))
        .filter(
            entity::background_job::Column::Status
                .is_in(["pending", "running", "awaiting_callback"]),
        )
        .order_by_asc(entity::background_job::Column::RunAt)
        .limit(25)
        .all(db)
        .await?;

    for job in due {
        advance(db, job).await?;
    }
    Ok(())
}

/// Advance a single job by one state transition.
async fn advance(
    db: &DatabaseConnection,
    job: entity::background_job::Model,
) -> Result<(), sea_orm::DbErr> {
    let now = Utc::now();
    let mut am: entity::background_job::ActiveModel = job.clone().into();
    am.attempts = Set(job.attempts + 1);
    am.updated_at = Set(now.into());

    match (job.kind.as_str(), job.status.as_str()) {
        // Background check / screening: pending -> awaiting external callback -> done.
        ("background_check" | "screening", "pending") => {
            am.status = Set("awaiting_callback".into());
            // Simulate provider latency before the callback "arrives".
            am.run_at = Set((now + Duration::seconds(6)).into());
            tracing::info!(job = %job.id, "screening submitted, awaiting callback");
        }
        ("background_check" | "screening", "awaiting_callback") => {
            am.status = Set("completed".into());
            am.result = Set(Some(json!({
                "cleared": true,
                "credit_band": "good",
                "eviction_records": 0,
                "completed_at": now.to_rfc3339(),
            })));
            tracing::info!(job = %job.id, "screening completed");
        }
        // Automated email: fire-and-complete.
        ("auto_email", _) => {
            am.status = Set("completed".into());
            am.result = Set(Some(json!({ "sent": true, "sent_at": now.to_rfc3339() })));
            tracing::info!(job = %job.id, "auto email sent");
        }
        // Generic webhook wait resolves on first pickup past run_at.
        ("webhook_wait", _) => {
            am.status = Set("completed".into());
            am.result = Set(Some(json!({ "resolved": true })));
        }
        _ => {
            am.status = Set("completed".into());
        }
    }

    am.update(db).await?;
    Ok(())
}
