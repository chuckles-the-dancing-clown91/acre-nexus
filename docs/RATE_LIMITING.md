# Rate limiting

Abuse protection for the API (roadmap Phase 8 GA hardening, issue #67).
Implemented as a Rocket fairing in `backend/crates/api/src/ratelimit.rs`.

## How it works

A **fixed-window** counter (60-second windows) attributes each request to a
caller and counts it. Two buckets keep brute-force surfaces separate from
ordinary traffic:

| Bucket | Applies to | Default limit | Env override |
|--------|-----------|---------------|--------------|
| `auth` | `/auth/login`, `/auth/refresh` | 10 / min | `RATE_LIMIT_AUTH_PER_MIN` |
| `gen`  | everything else | 300 / min | `RATE_LIMIT_PER_MIN` |

The limiter can be turned off entirely with `RATE_LIMIT_ENABLED=false`.

**Caller identity** is the API key (`X-Api-Key`) when present, otherwise the
client IP — read from `X-Real-IP` / `X-Forwarded-For` (first hop) when behind a
proxy, else the socket peer. Anonymous callers with no resolvable address share
one bucket.

**Exempt paths** (never counted): `/health`, `/openapi.json`, the Swagger /
RapiDoc explorers, the internal reject route, and CORS preflight (`OPTIONS`).

## Responses

Every tracked response carries:

- `X-RateLimit-Limit` — the bucket's per-window allowance
- `X-RateLimit-Remaining` — remaining allowance in the current window

When the window is exhausted the fairing reroutes the request to an internal
handler that returns **`429 Too Many Requests`** with a `Retry-After` header
(seconds until the window resets) and the standard error envelope, so clients
parse it exactly like any other API error:

```json
{ "error": { "code": "rate_limited", "message": "Too many requests — slow down and retry shortly." } }
```

## Scope & limitations

- Counters are **in-memory, per-process** — correct for a single instance. A
  multi-instance deployment should back the window store with a shared cache
  (e.g. Redis); the `hit()` logic is isolated to make that swap small.
- The window map is pruned opportunistically once it grows past 10,000 distinct
  callers, so memory stays bounded under a spray of unique IPs.
- Limits are per-caller-per-bucket, not per-tenant; per-plan quotas would layer
  on top and are out of scope here.
