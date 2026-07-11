//! **Request rate limiting** (roadmap Phase 8 GA hardening, issue #67).
//!
//! A fixed-window limiter attached as a Rocket fairing. Each inbound request is
//! attributed to a caller — its API key (`X-Api-Key`) if present, otherwise its
//! client IP — and counted within a rolling 60-second window. Two buckets keep
//! brute-force surfaces separate from ordinary traffic: authentication
//! endpoints (`/auth/login`, `/auth/refresh`) get a tight limit, everything else
//! a generous one. Health checks, the OpenAPI explorers, and CORS preflight are
//! exempt.
//!
//! On breach the fairing reroutes the request to an internal reject handler that
//! returns `429 Too Many Requests` (same JSON envelope as [`crate::error`]).
//! Every tracked response carries `X-RateLimit-Limit` / `X-RateLimit-Remaining`,
//! and a throttled one adds `Retry-After`.
//!
//! The counters are in-memory and per-process — correct for a single instance;
//! a multi-instance deployment would back this with a shared store (Redis). Env:
//! `RATE_LIMIT_ENABLED` (default true), `RATE_LIMIT_PER_MIN` (default 300),
//! `RATE_LIMIT_AUTH_PER_MIN` (default 10).

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::uri::Origin;
use rocket::http::{ContentType, Header, Status};
use rocket::request::Request;
use rocket::response::{self, Responder};
use rocket::{Data, Response};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Length of the fixed counting window.
const WINDOW: Duration = Duration::from_secs(60);
/// Where a throttled request is rerouted (an internal, unadvertised path).
const REJECT_PATH: &str = "/__rate_limited";
/// Prune the window map once it grows past this many distinct callers.
const PRUNE_AT: usize = 10_000;

/// Per-caller counting window.
struct Window {
    count: u32,
    start: Instant,
}

/// The decision for a request, stashed in request-local state so the response
/// fairing can emit the standard `X-RateLimit-*` headers.
#[derive(Clone, Copy, Default)]
struct RateInfo {
    /// 0 means "not tracked" (exempt / limiter disabled) — no headers emitted.
    limit: u32,
    remaining: u32,
    retry_after: u64,
    limited: bool,
}

pub struct RateLimiter {
    enabled: bool,
    general_per_min: u32,
    auth_per_min: u32,
    windows: Mutex<HashMap<String, Window>>,
}

impl RateLimiter {
    pub fn from_env() -> Self {
        let flag = |k: &str, d: bool| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(d)
        };
        let num = |k: &str, d: u32| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(d)
        };
        RateLimiter {
            enabled: flag("RATE_LIMIT_ENABLED", true),
            general_per_min: num("RATE_LIMIT_PER_MIN", 300),
            auth_per_min: num("RATE_LIMIT_AUTH_PER_MIN", 10),
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Count one hit for `key` against `limit`. Returns the remaining allowance
    /// and, when the window is exhausted, the seconds until it resets.
    fn hit(&self, key: &str, limit: u32) -> (u32, Option<u64>) {
        let now = Instant::now();
        let mut map = self.windows.lock().unwrap_or_else(|e| e.into_inner());

        // Opportunistic prune of expired windows so the map can't grow forever.
        if map.len() > PRUNE_AT {
            map.retain(|_, w| now.duration_since(w.start) < WINDOW);
        }

        let w = map.entry(key.to_string()).or_insert(Window {
            count: 0,
            start: now,
        });
        if now.duration_since(w.start) >= WINDOW {
            w.count = 0;
            w.start = now;
        }
        w.count += 1;
        if w.count > limit {
            let elapsed = now.duration_since(w.start).as_secs();
            let retry = WINDOW.as_secs().saturating_sub(elapsed).max(1);
            (0, Some(retry))
        } else {
            (limit - w.count, None)
        }
    }
}

/// Paths that are never rate-limited (monitoring + docs + preflight).
fn is_exempt(path: &str) -> bool {
    path == "/health"
        || path == "/metrics"
        || path == "/openapi.json"
        || path == REJECT_PATH
        || path.starts_with("/swagger-ui")
        || path.starts_with("/rapidoc")
}

/// Whether a path is an authentication endpoint (the tight bucket).
fn is_auth(path: &str) -> bool {
    path == "/auth/login" || path == "/auth/refresh"
}

/// Attribute a request to a caller: its API key if present, else its client IP,
/// else a shared `anon` bucket.
fn caller_id(req: &Request<'_>) -> String {
    if let Some(key) = req.headers().get_one("X-Api-Key") {
        return format!("key:{key}");
    }
    // Honour a fronting proxy's forwarded client address before the socket peer.
    if let Some(fwd) = req
        .headers()
        .get_one("X-Real-IP")
        .or_else(|| req.headers().get_one("X-Forwarded-For"))
    {
        let first = fwd.split(',').next().unwrap_or(fwd).trim();
        if !first.is_empty() {
            return format!("ip:{first}");
        }
    }
    match req.client_ip() {
        Some(ip) => format!("ip:{ip}"),
        None => "anon".to_string(),
    }
}

#[rocket::async_trait]
impl Fairing for RateLimiter {
    fn info(&self) -> Info {
        Info {
            name: "Rate limiter",
            kind: Kind::Request | Kind::Response,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        if !self.enabled {
            return;
        }
        let path = req.uri().path().to_string();
        if req.method() == rocket::http::Method::Options || is_exempt(&path) {
            return;
        }

        let auth = is_auth(&path);
        let limit = if auth {
            self.auth_per_min
        } else {
            self.general_per_min
        };
        let bucket = if auth { "auth" } else { "gen" };
        let key = format!("{bucket}:{}", caller_id(req));

        let (remaining, retry) = self.hit(&key, limit);
        let info = RateInfo {
            limit,
            remaining,
            retry_after: retry.unwrap_or(0),
            limited: retry.is_some(),
        };
        req.local_cache(|| info);

        if retry.is_some() {
            // Reroute to the internal reject handler; the method is preserved, so
            // a handler is registered for every verb we serve.
            if let Ok(origin) = Origin::parse_owned(REJECT_PATH.to_string()) {
                req.set_uri(origin);
            }
        }
    }

    async fn on_response<'r>(&self, req: &'r Request<'_>, res: &mut Response<'r>) {
        if !self.enabled {
            return;
        }
        let info = *req.local_cache(RateInfo::default);
        if info.limit == 0 {
            return;
        }
        res.set_header(Header::new("X-RateLimit-Limit", info.limit.to_string()));
        res.set_header(Header::new(
            "X-RateLimit-Remaining",
            info.remaining.to_string(),
        ));
        if info.limited {
            res.set_header(Header::new("Retry-After", info.retry_after.to_string()));
        }
    }
}

/// The body returned to a throttled caller — a `429` with the standard error
/// envelope. The `Retry-After` header is added by the fairing's response hook.
pub struct TooManyRequests;

impl<'r> Responder<'r, 'static> for TooManyRequests {
    fn respond_to(self, _req: &'r Request<'_>) -> response::Result<'static> {
        let body = rocket::serde::json::serde_json::json!({
            "error": {
                "code": "rate_limited",
                "message": "Too many requests — slow down and retry shortly."
            }
        })
        .to_string();
        Response::build()
            .status(Status::TooManyRequests)
            .header(ContentType::JSON)
            .sized_body(body.len(), std::io::Cursor::new(body))
            .ok()
    }
}

// Reject handlers for every verb the API serves; the fairing reroutes throttled
// requests here without changing their method.
#[rocket::get("/__rate_limited")]
pub fn reject_get() -> TooManyRequests {
    TooManyRequests
}
#[rocket::post("/__rate_limited")]
pub fn reject_post() -> TooManyRequests {
    TooManyRequests
}
#[rocket::put("/__rate_limited")]
pub fn reject_put() -> TooManyRequests {
    TooManyRequests
}
#[rocket::patch("/__rate_limited")]
pub fn reject_patch() -> TooManyRequests {
    TooManyRequests
}
#[rocket::delete("/__rate_limited")]
pub fn reject_delete() -> TooManyRequests {
    TooManyRequests
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter() -> RateLimiter {
        RateLimiter {
            enabled: true,
            general_per_min: 3,
            auth_per_min: 2,
            windows: Mutex::new(HashMap::new()),
        }
    }

    #[test]
    fn allows_up_to_the_limit_then_blocks() {
        let rl = limiter();
        assert_eq!(rl.hit("ip:a", 3), (2, None));
        assert_eq!(rl.hit("ip:a", 3), (1, None));
        assert_eq!(rl.hit("ip:a", 3), (0, None));
        // Fourth hit in the window is throttled.
        let (remaining, retry) = rl.hit("ip:a", 3);
        assert_eq!(remaining, 0);
        assert!(retry.is_some());
        assert!(retry.unwrap() >= 1);
    }

    #[test]
    fn separate_callers_have_separate_windows() {
        let rl = limiter();
        for _ in 0..3 {
            assert!(rl.hit("ip:a", 3).1.is_none());
        }
        // A different caller is unaffected by the first's exhausted window.
        assert_eq!(rl.hit("ip:b", 3), (2, None));
    }

    #[test]
    fn exempt_and_auth_path_classification() {
        assert!(is_exempt("/health"));
        assert!(is_exempt("/swagger-ui/index.html"));
        assert!(!is_exempt("/properties"));
        assert!(is_auth("/auth/login"));
        assert!(is_auth("/auth/refresh"));
        assert!(!is_auth("/auth/me"));
    }
}
