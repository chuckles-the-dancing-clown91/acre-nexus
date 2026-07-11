//! A **test-only** module (compiled only under `#[cfg(test)]`) that owns a
//! deterministic `test_retry` job kind, so the integration suite can pin the
//! background-queue retry/backoff/terminal-failure contract (issue #28) without
//! depending on a real provider's behaviour.
//!
//! The job's payload controls it:
//! * `fail_until` (i64) — fail (transient retry) on attempts `1..=fail_until`,
//!   then succeed. Set it `>= max_attempts` to always fail (exhaust the budget).
//! * `retry_delay_secs` (i64, default 0) — the backoff each retry asks for.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use serde_json::json;

pub struct TestJobsModule;

/// The job kind this module owns.
pub const TEST_RETRY_KIND: &str = "test_retry";

#[rocket::async_trait]
impl PlatformModule for TestJobsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "test_jobs",
            name: "Test Jobs",
            description: "Deterministic background-job kind for integration tests.",
            permissions: &[],
            job_kinds: &[TEST_RETRY_KIND],
            default_enabled: true,
            preview: false,
        }
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        let job = ctx.job;
        // `job.attempts` is the count *before* this attempt; make it 1-based.
        let this_attempt = job.attempts + 1;
        let fail_until = job
            .payload
            .get("fail_until")
            .and_then(|v| v.as_i64())
            .unwrap_or(i64::MAX);
        let retry_delay = job
            .payload
            .get("retry_delay_secs")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if (this_attempt as i64) <= fail_until {
            Some(JobOutcome::retry(
                retry_delay,
                format!("simulated failure on attempt {this_attempt}"),
            ))
        } else {
            Some(JobOutcome::completed(
                json!({ "ok": true, "attempts": this_attempt }),
            ))
        }
    }
}
