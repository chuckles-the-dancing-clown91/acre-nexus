# Audit Logging

Acre keeps a complete, queryable trail of activity against the platform. It is
designed to answer two questions at once:

- **What happened on the API?** — every request, including reads.
- **What changed, and who changed it?** — every state-changing operation, with
  structured before/after detail.

Both feed a single table (`audit_log`) and are surfaced at `GET /admin/audit`
(gated by the `audit:read` permission) and in the platform **Audit log** viewer.

---

## Two levels of entry

### 1. Request events (the access log)

A single Rocket **fairing** — `audit::AuditFairing`, attached once in
`main.rs` — observes every request/response pair. For each (non-skipped) request
it records:

| Column | Meaning |
|--------|---------|
| `action` | always `http.request` |
| `method` | `GET`, `POST`, … |
| `path` | request path, e.g. `/properties/<id>` |
| `status_code` | HTTP status of the response |
| `duration_ms` | wall-clock handling time |
| `ip` | client IP, when resolvable |
| `request_id` | correlation id, also returned as the `X-Request-Id` response header |
| `principal_kind` | `user`, `api_token`, or `public` |
| `actor_user_id` / `tenant_id` | resolved from the JWT or API key |

Because it is a fairing, **coverage is automatic**: every current and future
endpoint is audited without touching the handler. The principal is resolved
best-effort by decoding the JWT or looking up the API key (see
`audit/actor.rs`).

**Skipped** (pure infrastructure noise, see `audit/skip.rs`): `OPTIONS`
preflight, `/health`, `/openapi.json`, `/favicon.ico`, and the `/swagger-ui` /
`/rapidoc` explorers.

### 2. Domain events (what changed)

Handlers additionally call `audit::record(...)` on every state change, with a
stable dotted action key and structured `metadata`. These are the
human-readable "what changed" entries. They leave the request-context columns
`NULL`.

Wired into **every** mutating API:

| Area | Actions |
|------|---------|
| Auth | `auth.login`, `auth.logout`, `auth.refresh`, `auth.switch_workspace` |
| Properties | `property.create`, `property.update`, `llc.create` |
| Leasing | `application.submit` (public), `application.update` |
| Settings | `theme.update`, `module.toggle` |
| Vendor tokens | `apitoken.create`, `apitoken.revoke` |
| IAM | `user.create`, `role.create`, `role.update`, `role.delete`, `pii.reveal` |

The full taxonomy lives in `audit/actions.rs`.

---

## Reliability

Both writers are **best-effort**: a failed audit insert is logged (`tracing`)
and swallowed — it can never block or fail the underlying operation. The
per-request write is additionally dispatched off the request path (a spawned
task), so auditing adds no latency to the response.

---

## Schema

`audit_log` (see `entity/audit_log.rs`) carries the original event columns
(`id`, `actor_user_id`, `action`, `target_type`, `target_id`, `tenant_id`,
`metadata`, `created_at`) plus the request-context columns added in
`m20240101_000006_audit_request` (`method`, `path`, `status_code`,
`request_id`, `ip`, `duration_ms`, `principal_kind`). The request columns are
nullable, so domain events and request events share one table cleanly. Indexed
by `created_at`, `actor_user_id`, `action`, `path`, and `principal_kind`.

---

## Module layout

The subsystem is split into single-responsibility files under `api/src/audit/`:

| File | Responsibility |
|------|----------------|
| `mod.rs` | module docs + re-exports |
| `actions.rs` | the action-key taxonomy |
| `record.rs` | the domain-event writer (`record`) |
| `request_log.rs` | the per-request writer used by the fairing |
| `actor.rs` | resolving the principal from a request |
| `skip.rs` | which paths are excluded from request auditing |
| `fairing.rs` | the Rocket `AuditFairing` that ties it together |

---

## Reading the trail

`GET /admin/audit?limit=&action=` returns the newest entries first (default 100,
max 500), with the actor's display name resolved and every column above. The
platform viewer (`/console/platform/audit`) renders domain events and request
events together, colour-coding HTTP status and labelling the principal kind, and
offers a client-side filter by action.
