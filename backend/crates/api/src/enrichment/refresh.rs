//! The nightly **public-records refresh**: once a day per workspace, the
//! properties whose records are older than the workspace's refresh interval
//! get the refreshable sources re-run (parcel, taxes, valuation, crime), a
//! few per night so a provider's quota lasts the month.

use super::source::{Source, ORCHESTRATOR_KIND};
use crate::modules::JobOutcome;
use crate::settings;
use chrono::{Duration, Utc};
use entity::prelude::{BackgroundJob, Property, PropertyDetail, Tenant};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub const REFRESH_KIND: &str = "property_data_refresh";
/// Properties refreshed per night per workspace.
pub const PER_NIGHT: usize = 10;

/// Make sure every workspace has its nightly refresh queued (idempotent).
pub async fn ensure_recurring_jobs(db: &DatabaseConnection) {
    let Ok(tenants) = Tenant::find().all(db).await else {
        return;
    };
    for t in tenants {
        ensure_job_for_tenant(db, t.id).await;
    }
}

pub async fn ensure_job_for_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    let live = BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(tenant_id))
        .filter(entity::background_job::Column::Kind.eq(REFRESH_KIND))
        .filter(entity::background_job::Column::Status.is_in([
            "pending",
            "running",
            "awaiting_callback",
        ]))
        .one(db)
        .await;
    if matches!(live, Ok(None)) {
        let _ = crate::scheduler::enqueue(db, tenant_id, REFRESH_KIND, json!({}), 120).await;
    }
}

/// The properties due a refresh: records older than `days`, oldest first.
pub async fn due(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    days: i64,
    limit: usize,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let cutoff = Utc::now() - Duration::days(days.max(1));
    let details: HashMap<Uuid, entity::property_detail::Model> = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|d| (d.property_id, d))
        .collect();
    let mut rows: Vec<(i64, Uuid)> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .order_by_asc(entity::property::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .filter_map(|p| {
            let last = details
                .get(&p.id)
                .and_then(|d| d.last_enriched_at)
                .map(|t| t.timestamp());
            match last {
                Some(ts) if ts > cutoff.timestamp() => None,
                Some(ts) => Some((ts, p.id)),
                None => Some((0, p.id)),
            }
        })
        .collect();
    rows.sort();
    Ok(rows.into_iter().take(limit).map(|(_, id)| id).collect())
}

/// Run tonight's refresh and sleep until tomorrow.
pub async fn handle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let days = settings::get_i64(db, job.tenant_id, settings::PROPERTY_DATA_REFRESH_DAYS).await;
    let summary = if days <= 0 {
        json!({ "skipped": "refresh is off" })
    } else {
        match due(db, job.tenant_id, days, PER_NIGHT).await {
            Ok(ids) => {
                let sources: Vec<&str> = Source::refreshable().iter().map(|s| s.as_str()).collect();
                let mut queued = 0;
                for pid in &ids {
                    if crate::scheduler::enqueue(
                        db,
                        job.tenant_id,
                        ORCHESTRATOR_KIND,
                        json!({ "property_id": pid.to_string(), "sources": sources, "reason": "nightly" }),
                        0,
                    )
                    .await
                    .is_ok()
                    {
                        queued += 1;
                    }
                }
                json!({ "queued": queued, "property_ids": ids })
            }
            Err(e) => json!({ "error": e.to_string() }),
        }
    };
    let mut out = JobOutcome::reschedule("pending", 24 * 3600);
    out.result = Some(summary);
    out
}
