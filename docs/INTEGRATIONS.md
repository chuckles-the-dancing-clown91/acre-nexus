# Integrations — the Phase 1 substrate

The cross-cutting plumbing every external integration rides on (roadmap
Phase 1, epic #5): encrypted credential storage, a typed provider framework
with inbound webhooks, an object-store-backed document service, and
transactional notifications — all landed behind one `integrations` platform
module.

Everything follows the shape the [property enrichment engine](PROPERTY_DATA.md)
proved out: **deterministic simulated implementations by default**, a slot for
the real integration behind a configuration switch, and all side effects riding
the durable `background_job` queue with retries + exponential backoff.

| Piece | Issue | Code |
| --- | --- | --- |
| Secrets / KMS | #15 | `api/src/secrets.rs`, `entity/src/secret.rs` |
| Provider trait + webhooks | #16 | `api/src/providers/` |
| Object storage + documents | #17 | `api/src/storage.rs`, `api/src/routes/documents/` |
| Notifications | #18 | `api/src/notify/`, `entity/src/notification.rs` |
| The `integrations` module | #19 | `api/src/modules/integrations.rs` |

The notification layer has since grown past #18's scope — configurable
delivery providers (Resend/SendGrid/Postmark, Twilio, Slack/Discord), Web
Push, and the in-app inbox live in [`NOTIFICATIONS.md`](NOTIFICATIONS.md).

## 1. Secrets / KMS (#15)

Encrypted credential storage for everything between "hardcoded platform env
var" and "per-tenant credential a PM admin pastes into settings".

- **`secret` table** — `key` (dotted, e.g. `stripe.api_key`), AES-256-GCM
  ciphertext + nonce, `last4` for masked display, `created_by`, `rotated_at`.
  `tenant_id` is `NULL` for platform-wide secrets; a tenant row with the same
  key shadows the platform default.
- **A dedicated key, not `PII_ENC_KEY`.** Values seal with the same primitives
  as PII (`api/src/pii.rs`) but under `SECRETS_ENC_KEY`, so a leaked provider
  credential and a leaked SSN stay independently rotatable blast radii. The
  key resolution **fails closed in production**: with `APP_ENV=production` and
  no valid `SECRETS_ENC_KEY`, the server refuses to boot (no dev-style
  derive-from-`JWT_SECRET` fallback).
- **Access** — `secrets::reveal(db, tenant_id, key)` is the only read path,
  server-side only; no HTTP response ever carries plaintext. The settings UI
  is write-only: set / rotate / delete, masked `last4` display.
- **RBAC** — gated by the new `integrations:manage` permission.
- **Audit** — `secret.set` / `secret.rotate` / `secret.delete` record the key
  name, never the value.
- **RLS** — tenant contexts may *read* platform-wide rows (they are the
  provider-client fallback) but can only *write* rows in their own tenant.

**Routes** — `GET/PUT /integrations/secrets`, `DELETE /integrations/secrets/<key>`.

## 2. Outbound provider + webhook framework (#16)

The platform's first real `Provider` **trait**, generalized out of the shape
the enrichment engine proved by convention:

```rust
#[async_trait]
pub trait Provider {
    type Request: Serialize;
    type Response: Serialize;
    fn key(&self) -> &'static str;
    async fn call(&self, ctx, req) -> Result<Self::Response, ProviderError>;      // real
    async fn simulate(&self, ctx, req) -> Result<Self::Response, ProviderError>;  // CI/demo
    async fn execute(&self, ctx, req) -> …  // routes call vs simulate
}
```

- **Sandbox-first by construction** — `execute()` uses `simulate()` unless the
  provider's key is listed in `LIVE_PROVIDERS` (comma-separated; `all`
  enables everything). `ProviderError` mirrors `EnrichmentError`'s
  single-variant newtype.
- **Generic job runner** — `providers::run(provider, ctx, job, req)` executes
  one call for a background job, audits it (`provider.call`), and translates
  errors into `JobOutcome::retry` (with the shared `backoff` formula,
  `4 × 2^attempts` clamped — the enrichment module now imports the same
  function) or a terminal `JobOutcome::failed` once `max_attempts` is spent.
- **Outbound HTTP client** — `providers::client::OutboundClient` wraps
  `reqwest` with per-provider base URL + bearer auth resolved through
  `secrets::reveal`.
- **Inbound webhooks** — `POST /webhooks/<provider>` (tenant from `X-Tenant`
  or `?tenant=<slug>`; providers are configured with the full URL). The raw
  body's `X-Acre-Signature: sha256=<hex>` header is verified **constant-time**
  (HMAC-SHA256 via the `hmac` crate) against the signing secret stored under
  `webhook.<provider>.secret`. A verified event is **enqueued as a
  `webhook_event` background job** — never handled synchronously — so inbound
  events get the same durability/retry/audit trail as everything else, and is
  audited as `webhook.received`. No configured secret is indistinguishable
  from a bad signature (`401`).

## 3. Object storage + document service (#17)

One file service shared by everything that stores documents (e-signed PDFs,
data rooms, rehab photos, notices, media).

- **`document` table** — polymorphic `owner_type` (`property` | `lease` |
  `application` | `entity` | `deal` | `unit` | `maintenance_ticket` |
  `tenant`) + `owner_id`, filename, MIME type, size, SHA-256 `checksum`,
  `version` + `previous_version_id` (re-uploading the same filename against
  the same owner creates the next version — history is never destroyed),
  `retention_expires_at`, `created_by`. Blobs are opaque objects keyed by
  `storage_key` (`{tenant_id}/{document_id}`); the row is the only source of
  truth for metadata.
- **Two backends** (`STORAGE_BACKEND`):
  - `local` (default) — blobs under `STORAGE_DIR`, served by the
    `/storage/local/…` routes with expiring HMAC-signed URLs. What dev/CI
    exercises end to end.
  - `s3` — any S3-compatible store (AWS / R2 / MinIO) via AWS **SigV4 query
    presigning** implemented directly on `hmac` + `sha2` (no SDK dependency)
    and unit-tested against the worked example in the AWS documentation.
- **Signed-URL access** — the API returns short-lived, permission-checked
  URLs for upload (`PUT`) and download (`GET`); it never proxies S3 bytes.
- **Retention** — a `document_retention` job per retained document rides the
  queue and hard-deletes blob + row once `retention_expires_at` passes.
- **Audit** — `document.upload`, `document.download` (the fact a URL was
  issued, not the content — the `pii.reveal` discipline), `document.delete`.
- **RBAC** — `document:read` (list/download) and `document:manage`
  (upload/delete).

**Routes** — `POST /documents` (returns the row + a signed `PUT` URL),
`GET /documents?owner_type&owner_id`, `GET /documents/<id>/download` (returns
`{ url, expires_at }`), `DELETE /documents/<id>`.

## 4. Notifications (#18)

`auto_email` was a fire-and-complete stub owned by `leasing`; it is now real,
with the `{ "template": …, "to": … }` payload contract unchanged and ownership
moved to `integrations` (reminders, renewal notices, and statutory notices all
send mail and have nothing to do with leasing). A sibling `auto_sms` kind
joins it.

- **`notification` table** — channel, template key, recipient, rendered
  subject/body (the history trail), `status` (`queued` → `sent` | `failed`),
  `provider_message_id`, linked `background_job_id`.
- **Templating** — the same `{placeholder}` interpolation as lease documents
  (`leasedoc::interpolate`; no templating crate). Platform defaults live in
  `api/src/notify.rs`; tenants override per key via
  `theme.notification_templates` (sibling to `legal_templates`) — either a
  body string or `{ "subject": …, "body": …, "sms": … }`, merged field by
  field.
- **Delivery** — `EmailProvider` / `SmsProvider` implement the #16 trait. The
  simulated senders are what dev/CI exercise; the real ESP connector (#62)
  plugs into `call()` without touching any call site.
- **Idempotency** — payloads carrying owner context get a natural key
  (`channel:template:owner_type:owner_id:trigger`, unique per tenant), so a
  retried job or duplicate trigger can't double-send; legacy payloads dedupe
  per job.
- **Audit** — `notification.send` with template + channel + status, never the
  rendered body.

## 5. Wiring it together: the integrations module (#19)

All of the above surfaces through one new `PlatformModule`
(`modules/integrations.rs`), registered with one line in
`modules::registry()` per the [module contract](MODULES.md):

- **Manifest** — key `integrations`; permissions `integrations:manage`,
  `document:read`, `document:manage`; job kinds `auto_email`, `auto_sms`,
  `webhook_event`, `document_retention`. **On by default**
  (`default_enabled: true`, `preview: false`) — unlike `flips`, this is
  foundational plumbing every tenant needs.
- **Migration** — `m20240101_000018_integrations` adds `secret`, `document`,
  `notification` (+ indexes, enforced RLS) and the
  `theme.notification_templates` column in one pass.
- **Routes** — `routes/integrations/` and `routes/documents/`, one handler
  per file + `dto.rs`, mounted by the module's `api()`.
- **Frontend** — `/console/integrations` (credential vault, documents,
  notification log), registered in `frontend/src/modules/registry.ts` with
  its nav entry gated by `integrations:manage`.

### Configuration

```bash
SECRETS_ENC_KEY=   # 64 hex chars; REQUIRED in prod (fail closed), derived in dev
APP_ENV=development
STORAGE_BACKEND=local            # or "s3"
STORAGE_DIR=./data/objects       # local backend
PUBLIC_API_URL=http://localhost:8000
S3_BUCKET= S3_REGION= S3_ENDPOINT= AWS_ACCESS_KEY_ID= AWS_SECRET_ACCESS_KEY=
LIVE_PROVIDERS=                  # e.g. "email,sms" or "all"; empty = simulate
```

### Definition of Done (epic #5) — how to see it work

1. **Documents**: upload a file from `/console/integrations` against any
   record, download it via the signed URL (it expires), re-upload the same
   filename → version 2 linked to version 1; upload/download/delete all land
   on the audit trail.
2. **Webhooks**: store a signing secret under `webhook.test.secret`, then
   `POST /webhooks/test?tenant=<slug>` with an `X-Acre-Signature` HMAC of the
   body → `202`-style `{ received, job_id }` and the job round-trips through
   the queue to `completed`.
3. **Notifications**: approve an application → the `auto_email` job renders
   the `application_approved` template, the simulated provider "sends" it,
   the notification row flips to `sent`, and the send is audited.
4. **Secrets**: a credential stored from the console is encrypted under the
   dedicated key, readable server-side via `secrets::reveal`, and appears in
   no API response.
