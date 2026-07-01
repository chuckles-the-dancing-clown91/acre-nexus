//! The Rocket **audit fairing** — the single wiring point that makes the audit
//! log comprehensive.
//!
//! Attached once in [`crate::main`], it observes every request/response pair:
//! * `on_request` resolves the principal and starts a timer (stashed in the
//!   request's local cache),
//! * `on_response` computes latency + status, stamps an `X-Request-Id` response
//!   header, and writes the entry off the request path via a spawned task so the
//!   client is never slowed by the audit insert.
//!
//! Infrastructure noise is filtered by [`super::skip`].

use super::actor::{self, ResolvedActor};
use super::request_log::{self, RequestRecord};
use super::skip;
use crate::state::AppState;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Data, Request, Response};
use std::time::Instant;
use uuid::Uuid;

/// Per-request state stashed between `on_request` and `on_response`.
struct Trace {
    started: Instant,
    actor: ResolvedActor,
    request_id: Uuid,
}

/// The current request's audit correlation id, if the fairing traced it (i.e.
/// the path wasn't skipped — see [`super::skip`]). Lets any code holding a
/// `&Request` — notably [`crate::error::ApiError`]'s `Responder` impl — tag its
/// `tracing` logs with the same id that ends up in the `audit_log` row, so a log
/// line and its audit entry can be joined by `request_id`.
pub(crate) fn current_request_id(req: &Request<'_>) -> Option<Uuid> {
    req.local_cache(|| None::<Trace>)
        .as_ref()
        .map(|t| t.request_id)
}

/// Records every (non-skipped) HTTP request to the audit log.
pub struct AuditFairing;

#[rocket::async_trait]
impl Fairing for AuditFairing {
    fn info(&self) -> Info {
        Info {
            name: "Audit logger",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        let Some(state) = req.rocket().state::<AppState>() else {
            return;
        };
        if skip::should_skip(req.method().as_str(), &req.uri().path().to_string()) {
            return;
        }
        let actor = actor::resolve(req, state).await;
        let trace = Trace {
            started: Instant::now(),
            actor,
            request_id: Uuid::new_v4(),
        };
        // Stash for `on_response`; keyed by type, retrieved as `Option<Trace>`.
        req.local_cache(|| Some(trace));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let Some(trace) = req.local_cache(|| None::<Trace>) else {
            return; // skipped in on_request (or no state)
        };

        // Correlation id back to the caller.
        res.set_header(Header::new("X-Request-Id", trace.request_id.to_string()));

        let Some(state) = req.rocket().state::<AppState>() else {
            return;
        };

        let rec = RequestRecord {
            actor: trace.actor.clone(),
            method: req.method().as_str().to_string(),
            path: req.uri().path().to_string(),
            status_code: res.status().code as i32,
            request_id: trace.request_id,
            ip: req.client_ip().map(|ip| ip.to_string()),
            duration_ms: trace.started.elapsed().as_millis() as i64,
        };

        // Write off the request path so auditing never adds latency.
        let db = state.db.clone();
        tokio::spawn(async move { request_log::write(&db, rec).await });
    }
}
