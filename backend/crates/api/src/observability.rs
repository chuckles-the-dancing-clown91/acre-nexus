//! **Observability** (roadmap Phase 8 GA hardening) — request metrics + probes.
//!
//! A process-global [`Metrics`] registry records every request's method, status,
//! and latency (no path label — bounded cardinality). A fairing feeds it; three
//! endpoints expose it:
//!
//! * `GET /metrics` — Prometheus text exposition (request counters + a latency
//!   histogram + in-flight + uptime + live DB gauges). Optionally protected by a
//!   scrape token (`METRICS_TOKEN`); exempt from rate limiting and auditing.
//! * `GET /health/ready` — readiness probe: `200` when the database answers,
//!   `503` otherwise (`/health` remains the liveness probe).
//! * `GET /platform/observability` — the same numbers as JSON for the staff
//!   console, gated by `platform:admin`.
//!
//! Counters are in-memory and per-process — the right granularity for a scrape
//! target; a Prometheus server aggregates across instances.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{BackgroundJob, Tenant};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{ContentType, Status};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use rocket::{get, Data, Response, State};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Upper bounds (seconds) for the request-latency histogram.
const BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Background-job statuses surfaced as gauges.
const JOB_STATES: &[&str] = &[
    "pending",
    "running",
    "awaiting_callback",
    "failed",
    "completed",
];

#[derive(Default)]
struct Inner {
    /// (method, status) → count.
    requests: HashMap<(String, u16), u64>,
    /// Cumulative histogram: `buckets[i]` = observations ≤ `BUCKETS[i]`.
    buckets: Vec<u64>,
    sum_secs: f64,
    count: u64,
}

/// The process-global request metrics registry.
pub struct Metrics {
    start: Instant,
    in_flight: AtomicI64,
    inner: Mutex<Inner>,
}

impl Metrics {
    fn new() -> Self {
        Metrics {
            start: Instant::now(),
            in_flight: AtomicI64::new(0),
            inner: Mutex::new(Inner {
                buckets: vec![0; BUCKETS.len()],
                ..Default::default()
            }),
        }
    }

    /// The singleton, created on first use.
    pub fn global() -> &'static Metrics {
        static REGISTRY: OnceLock<Metrics> = OnceLock::new();
        REGISTRY.get_or_init(Metrics::new)
    }

    fn record(&self, method: &str, status: u16, secs: f64) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *inner
            .requests
            .entry((method.to_string(), status))
            .or_insert(0) += 1;
        inner.count += 1;
        inner.sum_secs += secs;
        for (i, bound) in BUCKETS.iter().enumerate() {
            if secs <= *bound {
                inner.buckets[i] += 1;
            }
        }
    }

    fn uptime_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// A snapshot for the JSON summary.
    fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let errors: u64 = inner
            .requests
            .iter()
            .filter(|((_, status), _)| *status >= 500)
            .map(|(_, c)| *c)
            .sum();
        let avg_ms = if inner.count > 0 {
            (inner.sum_secs / inner.count as f64) * 1000.0
        } else {
            0.0
        };
        Snapshot {
            total_requests: inner.count,
            server_errors: errors,
            avg_latency_ms: (avg_ms * 100.0).round() / 100.0,
            in_flight: self.in_flight.load(Ordering::Relaxed).max(0),
            uptime_secs: self.uptime_secs().round() as i64,
        }
    }
}

struct Snapshot {
    total_requests: u64,
    server_errors: u64,
    avg_latency_ms: f64,
    in_flight: i64,
    uptime_secs: i64,
}

// ---------------------------------------------------------------------------
// Fairing
// ---------------------------------------------------------------------------

/// Marker so `/metrics` and probes don't pollute their own numbers.
fn skip_path(path: &str) -> bool {
    path == "/metrics" || path == "/health" || path == "/health/ready"
}

#[derive(Clone, Copy)]
struct ReqStart(Instant);

pub struct MetricsFairing;

#[rocket::async_trait]
impl Fairing for MetricsFairing {
    fn info(&self) -> Info {
        Info {
            name: "Request metrics",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        if skip_path(&req.uri().path().to_string()) {
            return;
        }
        Metrics::global().in_flight.fetch_add(1, Ordering::Relaxed);
        req.local_cache(|| Some(ReqStart(Instant::now())));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let Some(ReqStart(start)) = *req.local_cache(|| None::<ReqStart>) else {
            return;
        };
        let m = Metrics::global();
        m.in_flight.fetch_sub(1, Ordering::Relaxed);
        m.record(
            req.method().as_str(),
            res.status().code,
            start.elapsed().as_secs_f64(),
        );
    }
}

// ---------------------------------------------------------------------------
// Live DB gauges
// ---------------------------------------------------------------------------

async fn db_gauges(db: &sea_orm::DatabaseConnection) -> (i64, HashMap<String, i64>) {
    let tenants = Tenant::find().count(db).await.unwrap_or(0) as i64;
    let mut jobs = HashMap::new();
    for state in JOB_STATES {
        let n = BackgroundJob::find()
            .filter(entity::background_job::Column::Status.eq(*state))
            .count(db)
            .await
            .unwrap_or(0) as i64;
        jobs.insert((*state).to_string(), n);
    }
    (tenants, jobs)
}

// ---------------------------------------------------------------------------
// /metrics (Prometheus text)
// ---------------------------------------------------------------------------

/// Scrape authorization: if `METRICS_TOKEN` is set, require a matching bearer
/// token; if unset (dev), allow. Keeps the endpoint machine-scrapable without a
/// JWT while still lockable in production.
pub struct ScrapeAuth;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ScrapeAuth {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let Ok(expected) = std::env::var("METRICS_TOKEN") else {
            return Outcome::Success(ScrapeAuth); // open when unset
        };
        if expected.is_empty() {
            return Outcome::Success(ScrapeAuth);
        }
        let provided = req
            .headers()
            .get_one("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "));
        if provided == Some(expected.as_str()) {
            Outcome::Success(ScrapeAuth)
        } else {
            Outcome::Error((Status::Unauthorized, ()))
        }
    }
}

/// A `text/plain; version=0.0.4` Prometheus exposition body.
pub struct PromText(String);

impl<'r> rocket::response::Responder<'r, 'static> for PromText {
    fn respond_to(self, _req: &'r Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .header(ContentType::new("text", "plain").with_params(("version", "0.0.4")))
            .sized_body(self.0.len(), std::io::Cursor::new(self.0))
            .ok()
    }
}

fn render_prometheus(m: &Metrics, tenants: i64, jobs: &HashMap<String, i64>) -> String {
    let inner = m.inner.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = String::new();

    out.push_str("# HELP acre_http_requests_total Total HTTP requests by method and status.\n");
    out.push_str("# TYPE acre_http_requests_total counter\n");
    let mut keys: Vec<_> = inner.requests.iter().collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    for ((method, status), count) in keys {
        let _ = writeln!(
            out,
            "acre_http_requests_total{{method=\"{method}\",status=\"{status}\"}} {count}"
        );
    }

    out.push_str("# HELP acre_http_request_duration_seconds Request latency.\n");
    out.push_str("# TYPE acre_http_request_duration_seconds histogram\n");
    for (i, bound) in BUCKETS.iter().enumerate() {
        let _ = writeln!(
            out,
            "acre_http_request_duration_seconds_bucket{{le=\"{bound}\"}} {}",
            inner.buckets[i]
        );
    }
    let _ = writeln!(
        out,
        "acre_http_request_duration_seconds_bucket{{le=\"+Inf\"}} {}",
        inner.count
    );
    let _ = writeln!(
        out,
        "acre_http_request_duration_seconds_sum {}",
        inner.sum_secs
    );
    let _ = writeln!(
        out,
        "acre_http_request_duration_seconds_count {}",
        inner.count
    );

    out.push_str("# HELP acre_http_requests_in_flight In-flight requests.\n");
    out.push_str("# TYPE acre_http_requests_in_flight gauge\n");
    let _ = writeln!(
        out,
        "acre_http_requests_in_flight {}",
        m.in_flight.load(Ordering::Relaxed).max(0)
    );

    out.push_str("# HELP acre_process_uptime_seconds Seconds since boot.\n");
    out.push_str("# TYPE acre_process_uptime_seconds gauge\n");
    let _ = writeln!(
        out,
        "acre_process_uptime_seconds {}",
        m.uptime_secs() as u64
    );

    out.push_str("# HELP acre_tenants_total Provisioned client workspaces.\n");
    out.push_str("# TYPE acre_tenants_total gauge\n");
    let _ = writeln!(out, "acre_tenants_total {tenants}");

    out.push_str("# HELP acre_background_jobs Background jobs by status.\n");
    out.push_str("# TYPE acre_background_jobs gauge\n");
    for state in JOB_STATES {
        let n = jobs.get(*state).copied().unwrap_or(0);
        let _ = writeln!(out, "acre_background_jobs{{status=\"{state}\"}} {n}");
    }

    out
}

/// `GET /metrics` — Prometheus exposition (see module docs).
#[get("/metrics")]
pub async fn metrics(state: &State<AppState>, _auth: ScrapeAuth) -> PromText {
    let (tenants, jobs) = db_gauges(&state.db).await;
    PromText(render_prometheus(Metrics::global(), tenants, &jobs))
}

// ---------------------------------------------------------------------------
// /health/ready
// ---------------------------------------------------------------------------

/// `GET /health/ready` — readiness probe (`503` if the database is unreachable).
#[get("/health/ready")]
pub async fn readiness(state: &State<AppState>) -> (Status, Json<serde_json::Value>) {
    match state.db.ping().await {
        Ok(_) => (
            Status::Ok,
            Json(serde_json::json!({ "ready": true, "db": "ok" })),
        ),
        Err(e) => {
            tracing::warn!("readiness: db ping failed: {e}");
            (
                Status::ServiceUnavailable,
                Json(serde_json::json!({ "ready": false, "db": "unreachable" })),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// /platform/observability (JSON for the console)
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct ObservabilityResp {
    pub uptime_secs: i64,
    pub total_requests: i64,
    pub server_errors: i64,
    pub avg_latency_ms: f64,
    pub in_flight: i64,
    pub tenants: i64,
    pub jobs: HashMap<String, i64>,
}

/// `GET /platform/observability` — runtime health for the staff console.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/observability")]
pub async fn observability(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<ObservabilityResp>> {
    user.require(Permission::PlatformAdmin)?;
    let snap = Metrics::global().snapshot();
    let (tenants, jobs) = db_gauges(&state.db).await;
    Ok(Json(ObservabilityResp {
        uptime_secs: snap.uptime_secs,
        total_requests: snap.total_requests as i64,
        server_errors: snap.server_errors as i64,
        avg_latency_ms: snap.avg_latency_ms,
        in_flight: snap.in_flight,
        tenants,
        jobs,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_requests_and_histogram() {
        let m = Metrics::new();
        m.record("GET", 200, 0.003);
        m.record("GET", 200, 0.2);
        m.record("POST", 500, 1.5);
        let inner = m.inner.lock().unwrap();
        assert_eq!(inner.count, 3);
        assert_eq!(inner.requests[&("GET".into(), 200)], 2);
        assert_eq!(inner.requests[&("POST".into(), 500)], 1);
        // 0.003 ≤ 0.005 → in the first bucket; the 0.2 obs enters at le=0.25.
        assert_eq!(inner.buckets[0], 1);
        // le=0.25 (index 5) is cumulative: 0.003 and 0.2 both ≤ 0.25.
        assert_eq!(inner.buckets[5], 2);
    }

    #[test]
    fn snapshot_computes_error_and_latency() {
        let m = Metrics::new();
        m.record("GET", 200, 0.1);
        m.record("GET", 503, 0.3);
        let s = m.snapshot();
        assert_eq!(s.total_requests, 2);
        assert_eq!(s.server_errors, 1);
        assert_eq!(s.avg_latency_ms, 200.0);
    }
}
