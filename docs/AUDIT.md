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

Wired into **every** mutating API — including every membership/role/profile
mutation in IAM, which previously left the domain trail blank (fixed; see
below):

| Area | Actions |
|------|---------|
| Auth | `auth.login`, `auth.logout`, `auth.refresh`, `auth.switch_workspace` |
| Properties | `property.create`, `property.update`, `property.onboard`, `property.enrich`, `property.enrichment_run`, `llc.create` |
| Leasing | `application.submit` (public), `application.screened` (pipeline), `application.advance`, `application.convert`, `application.reuse`, `listing.create`, `listing.update`, `listing.sync` (pipeline), `lease.activate` (pipeline) |
| E-signature | `esign.send`, `esign.view`, `esign.sign`, `esign.decline`, `esign.remind`, `esign.complete`, `esign.void` (staff **and** the in-person-signing auto-void) |
| Documents | `document.upload` (incl. the pipeline-stored signed PDF), `document.stored` (tokenized blob PUT finalizing the row), `document.download`, `document.delete` |
| Notifications | `notification.send`, `notification.broadcast`, `notification.test`, `notification.read`, provider CRUD, template edits, `push.subscribe`/`push.unsubscribe` |
| Settings | `theme.update`, `module.toggle`, `setting.update` |
| Vendor tokens | `apitoken.create`, `apitoken.revoke` |
| IAM | `user.create`, `user.update`, `role.create`, `role.update`, `role.delete`, `role.assign`, `role.revoke`, `membership.add`, `membership.remove`, `profile.write`, `pii.reveal` |
| Assignments | `assignment.create`, `assignment.remove` |

The full taxonomy lives in `audit/actions.rs`.

**Sensitive metadata**: writers never put raw PII in `metadata`. `profile.write`
records only which fields were touched (`fields_set: [...]`), never their
values; `pii.reveal` likewise logs the *fact* of an SSN/gov-id read, not the
decrypted value.

### 3. Background-job mutations

Not every mutation happens inside an HTTP request — the Tokio [scheduler](../backend/crates/api/src/scheduler.rs)
runs jobs (enrichment, screening, automated email) with no request, and so no
actor, behind them. `property.enrich` (recorded when `POST
/properties/<id>/enrich` **enqueues** the job) only proves someone asked; it's
`property.enrichment_run` — recorded once per source in the job handler's
`record_run()`, actor `None` — that proves property data actually changed (or
didn't, and why: `metadata.status` is `succeeded` or `failed`). The same
actor-`None` discipline covers the whole leasing pipeline's automatic
mutations: `application.screened` when a background check lands its verdict,
`lease.activate` when signing flips the tenancy on, `listing.sync` when the
pipeline moves a listing (conversion → `Pending`, activation → `Leased`,
declined envelope → `Available`), `property.update` with
`trigger: "occupancy_sync"` when reconciliation changes occupancy or
availability status, `workflow.advance` with `trigger: "lease_signed"`,
`esign.void` with `trigger: "in_person_signing"`, and `document.upload` with
`source: "esign_completion"` when the signed PDF is stored (immediately or by
the deferred `esign_store_pdf` retry job).

---

## Reliability

Both writers are **best-effort**: a failed audit insert is logged (`tracing`)
and swallowed — it can never block or fail the underlying operation. The
per-request write is additionally dispatched off the request path (a spawned
task), so auditing adds no latency to the response.

---

## Logging ↔ audit correlation

The two systems are tied together by the same **`request_id`** the fairing
generates: it's stored in the request's `audit_log` row, returned as the
`X-Request-Id` response header, *and* available to any code holding a
`&Request` via `crate::audit::current_request_id(req)` (it reads the same
per-request cache slot the fairing populates in `on_request`). `error::ApiError`'s
`Responder` uses this to tag its `tracing::error!` calls for `Db`/`Internal`
failures with `request_id`, so a stack-trace-bearing log line can be joined
back to the exact audit row (and every other request-scoped fact — actor,
tenant, latency) by that id.

**Log format**: `tracing-subscriber` defaults to human-readable text for local
development. Set `LOG_FORMAT=json` to switch to newline-delimited JSON (one
object per line, with a `fields.request_id` entry on tagged lines) for shipping
to a log aggregator that can index/query on `request_id` alongside `audit_log`.
The level filter is still controlled by `RUST_LOG` (default `info,sqlx=warn`).

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

---

## Client-side logging (frontend)

The frontend has no telemetry SDK wired up (no Sentry/PostHog/etc. —
`frontend/src/lib/log.ts` is the one place to add one later); today "logging"
means making sure a failure is at least visible in the browser console instead
of vanishing, which is what used to happen at ~20 call sites that caught a
failed data load and did nothing with it.

- **`logError(context, err)`** (`lib/log.ts`) is the single logging call site —
  every fix below routes through it.
- **`request()`** (`lib/api.ts`), the one function every API call goes through,
  logs network-level failures (offline, DNS, CORS) centrally — the frontend
  analogue of the backend's request fairing, since there's no legitimate
  "expected" case for those the way there is for a 4xx response.
- **Secondary/best-effort data loads** (property intel, mortgages, workflow,
  units, leases, tickets, ownership, liens, the application/workflow catalog,
  tenant branding, …) now log their failure instead of silently leaving the
  panel empty. Expected non-errors (e.g. a 404 meaning "lease document not
  generated yet") are still handled without logging — only checked by status
  code, not assumed from a bare catch.
- **`QueryClient`** (`lib/query.tsx`) has a `QueryCache`/`MutationCache`
  `onError` safety net that logs any `useQuery`/`useMutation` failure a hook
  didn't already handle — visibility only, it doesn't add/duplicate toasts.
- **`error.tsx` / `global-error.tsx`** (`app/`) are Next.js App Router error
  boundaries: an uncaught render exception is logged and shown a "Try again"
  screen instead of a blank page. `global-error.tsx` covers failures in the
  root layout itself (the one place `error.tsx` can't reach) and renders
  standalone, since the layout that would style it is what failed.
- Deliberately left alone: a stale/expired session on `/auth/me` and a missing
  `tenant:manage` permission on the module list are both frequent, expected
  outcomes of normal use, not bugs — logging them as errors on every affected
  page load would be noise, not signal.
