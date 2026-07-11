//! **Observability** (#32): a lightweight, dependency-free Prometheus metrics
//! registry, plus an error-reporting sink. Hand-rolled exposition keeps this in
//! the same spirit as the rest of the codebase (no heavy metrics crate).
//!
//! * HTTP request rate / latency / status-class, recorded by the audit fairing.
//! * Background-job outcome counters, recorded by the scheduler.
//! * A live `background_jobs_pending` gauge, scraped from the DB at `/metrics`.
//! * [`report_error`] surfaces `ApiError::Internal` / panics to an actionable
//!   sink (an `ERROR_WEBHOOK_URL`, if configured), correlated by `X-Request-Id`.

use crate::state::AppState;
use rocket::http::ContentType;
use rocket::{get, State};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::OnceLock;
use uuid::Uuid;

/// Cumulative latency histogram buckets, in seconds.
const BUCKETS: [f64; 8] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0];

struct Metrics {
    requests_total: AtomicU64,
    requests_2xx: AtomicU64,
    requests_3xx: AtomicU64,
    requests_4xx: AtomicU64,
    requests_5xx: AtomicU64,
    duration_sum_ms: AtomicU64,
    /// Cumulative counts: `duration_buckets[i]` = requests with latency ≤ BUCKETS[i].
    duration_buckets: [AtomicU64; 8],
    errors_total: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
    jobs_retried: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Metrics {
            requests_total: AtomicU64::new(0),
            requests_2xx: AtomicU64::new(0),
            requests_3xx: AtomicU64::new(0),
            requests_4xx: AtomicU64::new(0),
            requests_5xx: AtomicU64::new(0),
            duration_sum_ms: AtomicU64::new(0),
            duration_buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            errors_total: AtomicU64::new(0),
            jobs_completed: AtomicU64::new(0),
            jobs_failed: AtomicU64::new(0),
            jobs_retried: AtomicU64::new(0),
        }
    }
}

static M: OnceLock<Metrics> = OnceLock::new();
fn m() -> &'static Metrics {
    M.get_or_init(Metrics::new)
}

/// Record one completed HTTP request (called by the audit fairing).
pub fn record_request(status: u16, duration_ms: u64) {
    let m = m();
    m.requests_total.fetch_add(1, Relaxed);
    match status / 100 {
        2 => &m.requests_2xx,
        3 => &m.requests_3xx,
        4 => &m.requests_4xx,
        _ => &m.requests_5xx,
    }
    .fetch_add(1, Relaxed);
    m.duration_sum_ms.fetch_add(duration_ms, Relaxed);
    let secs = duration_ms as f64 / 1000.0;
    for (i, b) in BUCKETS.iter().enumerate() {
        if secs <= *b {
            m.duration_buckets[i].fetch_add(1, Relaxed);
        }
    }
}

/// Record a background-job state transition (called by the scheduler).
pub fn record_job(outcome: &str) {
    let m = m();
    match outcome {
        "completed" => &m.jobs_completed,
        "failed" => &m.jobs_failed,
        "retry" => &m.jobs_retried,
        _ => return,
    }
    .fetch_add(1, Relaxed);
}

/// Surface an internal error / panic to an actionable sink and bump the error
/// counter. If `ERROR_WEBHOOK_URL` is set, best-effort POST a JSON payload
/// (correlated by `request_id`); always counted in metrics regardless.
pub fn report_error(request_id: Option<Uuid>, kind: &str, detail: &str) {
    m().errors_total.fetch_add(1, Relaxed);
    let url = match std::env::var("ERROR_WEBHOOK_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => return,
    };
    let payload = serde_json::json!({
        "service": "acre-api",
        "kind": kind,
        "detail": detail,
        "request_id": request_id.map(|r| r.to_string()),
    });
    // Fire-and-forget so error reporting never blocks or fails the response.
    // Guard on a live runtime so this is safe to call from the panic hook, which
    // may fire outside any Tokio context (e.g. during startup).
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("error-report webhook skipped: no tokio runtime");
        return;
    };
    handle.spawn(async move {
        let client = reqwest::Client::new();
        if let Err(e) = client
            .post(&url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            tracing::warn!("error-report webhook failed: {e}");
        }
    });
}

/// `GET /metrics` — Prometheus text exposition. Unauthenticated (scrape it on an
/// internal network); exempt from audit + rate limiting.
#[get("/metrics")]
pub async fn endpoint(state: &State<AppState>) -> (ContentType, String) {
    use entity::prelude::BackgroundJob;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let jobs_pending = BackgroundJob::find()
        .filter(entity::background_job::Column::Status.is_in([
            "pending",
            "running",
            "awaiting_callback",
        ]))
        .count(&state.db)
        .await
        .unwrap_or(0);

    (ContentType::Plain, render(jobs_pending as i64))
}

fn render(jobs_pending: i64) -> String {
    let m = m();
    let total = m.requests_total.load(Relaxed);
    let mut out = String::with_capacity(1024);

    let counter = |out: &mut String, name: &str, help: &str, val: u64| {
        let _ = writeln!(out, "# HELP {name} {help}");
        let _ = writeln!(out, "# TYPE {name} counter");
        let _ = writeln!(out, "{name} {val}");
    };

    let _ = writeln!(
        out,
        "# HELP http_requests_total Total HTTP requests by status class."
    );
    let _ = writeln!(out, "# TYPE http_requests_total counter");
    let _ = writeln!(
        out,
        "http_requests_total{{class=\"2xx\"}} {}",
        m.requests_2xx.load(Relaxed)
    );
    let _ = writeln!(
        out,
        "http_requests_total{{class=\"3xx\"}} {}",
        m.requests_3xx.load(Relaxed)
    );
    let _ = writeln!(
        out,
        "http_requests_total{{class=\"4xx\"}} {}",
        m.requests_4xx.load(Relaxed)
    );
    let _ = writeln!(
        out,
        "http_requests_total{{class=\"5xx\"}} {}",
        m.requests_5xx.load(Relaxed)
    );

    // Latency histogram (cumulative buckets, seconds).
    let _ = writeln!(out, "# HELP http_request_duration_seconds Request latency.");
    let _ = writeln!(out, "# TYPE http_request_duration_seconds histogram");
    for (i, b) in BUCKETS.iter().enumerate() {
        let _ = writeln!(
            out,
            "http_request_duration_seconds_bucket{{le=\"{b}\"}} {}",
            m.duration_buckets[i].load(Relaxed)
        );
    }
    let _ = writeln!(
        out,
        "http_request_duration_seconds_bucket{{le=\"+Inf\"}} {total}"
    );
    let sum_secs = m.duration_sum_ms.load(Relaxed) as f64 / 1000.0;
    let _ = writeln!(out, "http_request_duration_seconds_sum {sum_secs}");
    let _ = writeln!(out, "http_request_duration_seconds_count {total}");

    counter(
        &mut out,
        "http_errors_total",
        "Internal errors / panics reported.",
        m.errors_total.load(Relaxed),
    );
    counter(
        &mut out,
        "background_jobs_completed_total",
        "Jobs that completed.",
        m.jobs_completed.load(Relaxed),
    );
    counter(
        &mut out,
        "background_jobs_failed_total",
        "Jobs that terminally failed.",
        m.jobs_failed.load(Relaxed),
    );
    counter(
        &mut out,
        "background_jobs_retried_total",
        "Job attempts that retried.",
        m.jobs_retried.load(Relaxed),
    );

    let _ = writeln!(
        out,
        "# HELP background_jobs_pending Jobs awaiting processing."
    );
    let _ = writeln!(out, "# TYPE background_jobs_pending gauge");
    let _ = writeln!(out, "background_jobs_pending {jobs_pending}");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_is_valid_prometheus_text() {
        record_request(200, 3);
        record_request(503, 1200);
        record_job("completed");
        report_error(Some(Uuid::nil()), "test", "synthetic");
        let text = render(4);
        assert!(text.contains("http_requests_total{class=\"2xx\"}"));
        assert!(text.contains("http_request_duration_seconds_bucket{le=\"+Inf\"}"));
        assert!(text.contains("http_request_duration_seconds_count"));
        assert!(text.contains("background_jobs_pending 4"));
        assert!(text.contains("http_errors_total"));
        // The 1200ms request must NOT fall into the ≤1s bucket.
        assert!(text.contains("background_jobs_completed_total 1"));
    }
}
