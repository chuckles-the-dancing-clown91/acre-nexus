//! **Property Intelligence** module — the rich per-property data surface and the
//! automation that fills it. It owns the enrichment job kinds and dispatches each
//! to the [`crate::enrichment`] engine, and contributes the `/properties/<id>/…`
//! intel/enrich/runs routes.
//!
//! The orchestrator job (`enrich_property`) fans out into one child job per
//! source so each fetch runs and retries on the durable queue independently.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::enrichment::{self, Source};
use crate::rbac::Permission;
use crate::routes::property_intel;
use crate::scheduler;
use entity::prelude::Property;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

pub struct EnrichmentModule;

#[rocket::async_trait]
impl PlatformModule for EnrichmentModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "property_intel",
            name: "Property Intelligence",
            description: "Parcel/county records, taxes, valuations (AVM), schools & utilities — \
                 fetched and validated automatically.",
            permissions: &[Permission::PropertyRead, Permission::PropertyWrite],
            job_kinds: enrichment::JOB_KINDS,
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            property_intel::get_intel::get_intel,
            property_intel::enrich::enrich,
            property_intel::list_enrichment::list_enrichment,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        let job = ctx.job;
        // Property + enrichment_run live in the property database; child jobs are
        // enqueued into background_job in the user database.
        let property_db = ctx.property_db;

        // Orchestrator: fan out into one child job per requested source.
        if job.kind == enrichment::ORCHESTRATOR_KIND {
            return Some(orchestrate(ctx.user_db, job).await);
        }

        // Otherwise this is a single-source job.
        let source = Source::from_job_kind(&job.kind)?;
        let property = match load_property(property_db, job).await {
            Ok(Some(p)) => p,
            Ok(None) => return Some(JobOutcome::failed("property not found")),
            Err(e) => return Some(JobOutcome::retry(backoff(job.attempts), e)),
        };

        match enrichment::runner::run_source(property_db, &property, source).await {
            Ok(summary) => {
                record_run(
                    property_db,
                    &property,
                    source,
                    "succeeded",
                    Some(job.id),
                    summary.clone(),
                )
                .await;
                Some(JobOutcome::completed(json!({
                    "source": source.as_str(),
                    "summary": summary,
                })))
            }
            Err(e) => {
                let attempts = job.attempts + 1;
                if attempts >= job.max_attempts {
                    record_run(
                        property_db,
                        &property,
                        source,
                        "failed",
                        Some(job.id),
                        json!({ "error": e.to_string() }),
                    )
                    .await;
                    Some(JobOutcome::failed(e.to_string()))
                } else {
                    Some(JobOutcome::retry(backoff(job.attempts), e.to_string()))
                }
            }
        }
    }
}

/// Exponential backoff (seconds) for transient enrichment failures.
fn backoff(attempts: i32) -> i64 {
    let exp = attempts.clamp(0, 6) as u32;
    4_i64 * 2_i64.pow(exp)
}

/// Parse the `property_id` from a job payload.
fn payload_property_id(job: &entity::background_job::Model) -> Option<Uuid> {
    job.payload
        .get("property_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Load the job's target property, scoped to the job's tenant.
async fn load_property<C: ConnectionTrait>(
    db: &C,
    job: &entity::background_job::Model,
) -> Result<Option<entity::property::Model>, String> {
    let pid = match payload_property_id(job) {
        Some(p) => p,
        None => return Ok(None),
    };
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
        .map_err(|e| format!("db error: {e}"))
}

/// Fan out the orchestrator job into one child job per requested source.
async fn orchestrate(db: &DatabaseConnection, job: &entity::background_job::Model) -> JobOutcome {
    let pid = match payload_property_id(job) {
        Some(p) => p,
        None => return JobOutcome::failed("orchestrator: missing property_id"),
    };

    // Sources requested (default: all).
    let requested: Vec<Source> = job
        .payload
        .get("sources")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter_map(Source::from_str)
                .collect()
        })
        .filter(|v: &Vec<Source>| !v.is_empty())
        .unwrap_or_else(|| Source::all().to_vec());

    let mut scheduled = Vec::new();
    for source in &requested {
        let enq = scheduler::enqueue(
            db,
            job.tenant_id,
            source.job_kind(),
            json!({ "property_id": pid.to_string() }),
            0,
        )
        .await;
        match enq {
            Ok(_) => scheduled.push(source.as_str()),
            Err(e) => tracing::error!("failed to enqueue {} job: {e}", source.as_str()),
        }
    }

    JobOutcome::completed(json!({ "scheduled": scheduled }))
}

/// Record an `enrichment_run` row (best-effort).
async fn record_run<C: ConnectionTrait>(
    db: &C,
    property: &entity::property::Model,
    source: Source,
    status: &str,
    job_id: Option<Uuid>,
    detail: serde_json::Value,
) {
    let row = entity::enrichment_run::ActiveModel {
        id: Set(Uuid::now_v7()),
        tenant_id: Set(property.tenant_id),
        property_id: Set(property.id),
        source: Set(source.as_str().to_string()),
        status: Set(status.to_string()),
        job_id: Set(job_id),
        provider: Set(source.provider().to_string()),
        detail: Set(Some(detail)),
        created_at: Set(chrono::Utc::now().into()),
    };
    if let Err(e) = row.insert(db).await {
        tracing::error!("failed to record enrichment_run: {e}");
    }
}
