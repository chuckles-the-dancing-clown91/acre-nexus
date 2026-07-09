# Observability

Runtime metrics and health probes for the API (roadmap Phase 8 GA hardening).
Implemented in `backend/crates/api/src/observability.rs`.

## Request metrics

A process-global registry, fed by a Rocket fairing, records every request's
**method**, **status**, and **latency** — no path label, so cardinality stays
bounded. The registry itself is in-memory and per-process (the right granularity
for a scrape target; a Prometheus server aggregates across instances).

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | — | Liveness probe (always `200` if the process is up). |
| GET | `/health/ready` | — | Readiness probe — pings the database; `200 {ready:true}` or `503 {ready:false}`. |
| GET | `/metrics` | scrape token (optional) | Prometheus text exposition. |
| GET | `/platform/observability` | `platform:admin` | The same numbers as JSON for the staff console. |

`/health`, `/health/ready`, and `/metrics` are exempt from rate limiting and
from the audit log (pure infrastructure traffic), and are excluded from their
own request metrics so a scrape doesn't inflate the counters.

### `/metrics` series

Prometheus text (`text/plain; version=0.0.4`):

- `acre_http_requests_total{method,status}` — counter
- `acre_http_request_duration_seconds` — histogram (`_bucket{le}` / `_sum` / `_count`)
- `acre_http_requests_in_flight` — gauge
- `acre_process_uptime_seconds` — gauge
- `acre_tenants_total` — gauge (live from the DB)
- `acre_background_jobs{status}` — gauge (live: pending / running / awaiting_callback / failed / completed)

### Scrape authorization

`/metrics` is protected by an optional bearer token: set `METRICS_TOKEN` and
scrapers must send `Authorization: Bearer <token>`; leave it unset (dev) and the
endpoint is open. This keeps the endpoint machine-scrapable without a JWT while
still lockable in production. `/health/ready` stays open for load-balancer
health checks.

## Console

The **Platform admin** page (`/console/platform`, staff-only) renders a **System
health** card from `/platform/observability`: uptime, total requests, average
latency, in-flight requests, and pending / failed background jobs, with a
`healthy` / `N server errors` badge.
