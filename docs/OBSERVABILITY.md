# Observability

Metrics, tracing, and error reporting (issue #32). Builds on the structured
`tracing` logs and the audit fairing (which already stamps every request with an
`X-Request-Id` and records method/path/status/latency).

## Metrics — `GET /metrics`

A hand-rolled Prometheus text exposition (no heavy metrics dependency), scraped
from an internal network. Unauthenticated, and exempt from auditing + rate
limiting so scrapes don't pollute the audit log or count against limits.

Exposed series:

| Metric | Type | Notes |
|---|---|---|
| `http_requests_total{class}` | counter | requests by status class (`2xx`…`5xx`) |
| `http_request_duration_seconds` | histogram | latency, cumulative buckets + sum + count |
| `http_errors_total` | counter | internal errors / panics reported |
| `background_jobs_completed_total` | counter | jobs that completed |
| `background_jobs_failed_total` | counter | jobs that terminally failed |
| `background_jobs_retried_total` | counter | job attempts that retried |
| `background_jobs_pending` | gauge | jobs awaiting processing (scraped live from the DB) |

Request metrics are recorded by the audit fairing (`audit::fairing`); job metrics
by the scheduler (`scheduler::advance`). A metrics spike on `http_errors_total`
or a status class can be traced back to specific `audit_log` rows via the
`request_id` those rows share with the error logs.

## Error reporting

`ApiError::Internal` / `ApiError::Db` responses and unhandled **panics** are
surfaced to an actionable sink, not just logged:

- Both bump `http_errors_total`.
- If `ERROR_WEBHOOK_URL` is set, a best-effort JSON payload is POSTed to it
  (`{ service, kind, detail, request_id }`) — fire-and-forget, so reporting never
  blocks or fails the response. The `request_id` correlates the alert with the
  audit log and the `X-Request-Id` the client received.
- A process-wide panic hook reports panics through the same path before delegating
  to the default hook.

Wire `ERROR_WEBHOOK_URL` to a Sentry/Rollbar-style intake or a Slack incoming
webhook (the payload is plain JSON).

## Definition of done

A deliberately-triggered internal error (or panic) increments `http_errors_total`
at `/metrics` **and** POSTs to the error webhook within seconds, carrying the
`request_id` that ties it back to the audit-log row for that request.

## Config

| Var | Effect |
|---|---|
| `LOG_FORMAT=json` | newline-delimited JSON logs (for aggregators) |
| `NO_COLOR` | never colour the console output (it's already plain when stdout isn't a terminal) |
| `ERROR_WEBHOOK_URL` | error/panic reports POSTed here (unset = metrics only) |

## Request log

Each request is one line, `METHOD /path status Nms requestid`, from
`httplog.rs`; the request id joins it to its `audit_log` row. 4xx lines are
warnings and 5xx are errors. Preflights, `/health`, `/metrics` and the unread
badge poll are left out unless they fail. Rocket's own five-line-per-request
output is turned off (`rocket::server=off`), colour is off unless stdout is a
terminal, and `RUST_LOG` still overrides all of it.
