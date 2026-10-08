//! One log line per request, instead of Rocket's five: method, path, status,
//! how long it took, and the request id that joins it to the audit log.
//! Preflights and the routine polls (`/health`, `/metrics`, the unread badge)
//! are left out unless they fail.

use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Data, Request, Response};
use std::time::Instant;

pub struct HttpLog;

struct Started(Instant);

/// Requests that happen on a timer and would drown everything else.
fn is_chatter(method: &str, path: &str) -> bool {
    method == "OPTIONS"
        || path == "/health"
        || path == crate::routes::solnyxus::HEALTH_PATH
        || path == "/metrics"
        || path == "/notifications/unread_count"
}

#[rocket::async_trait]
impl Fairing for HttpLog {
    fn info(&self) -> Info {
        Info {
            name: "Request log",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        req.local_cache(|| Started(Instant::now()));
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        let ms = req
            .local_cache(|| Started(Instant::now()))
            .0
            .elapsed()
            .as_millis();
        let status = res.status().code;
        let method = req.method().as_str();
        let path = req.uri().path().to_string();
        let id = crate::audit::current_request_id(req)
            .map(|i| i.simple().to_string()[..8].to_string())
            .unwrap_or_default();
        if status >= 500 {
            tracing::error!("{method} {path} {status} {ms}ms {id}");
        } else if status >= 400 && !(method == "OPTIONS") {
            tracing::warn!("{method} {path} {status} {ms}ms {id}");
        } else if !is_chatter(method, &path) {
            tracing::info!("{method} {path} {status} {ms}ms {id}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_chatter;

    #[test]
    fn quiet_routes() {
        assert!(is_chatter("OPTIONS", "/leases"));
        assert!(is_chatter("GET", "/notifications/unread_count"));
        assert!(!is_chatter("GET", "/leases"));
        assert!(!is_chatter("POST", "/health/deep"));
    }
}
