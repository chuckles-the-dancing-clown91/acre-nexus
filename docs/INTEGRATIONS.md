# Integration Substrate (Phase 1 — design)

> **Status: design, not yet implemented.** This is the spec for [#5](../../../issues/5)
> and its five sub-issues ([#15](../../../issues/15)–[#19](../../../issues/19)). Unlike
> the other `docs/*.md` files, which document shipped subsystems, this one exists so
> whoever picks up Phase 1 doesn't have to reverse-engineer the plan from five issue
> bodies. Update it as the real implementation diverges from this design.

Every phase after this one needs the same four things: somewhere safe to keep a
third party's credential, a uniform way to call out to (and hear back from) a
third party, somewhere to put a file, and a way to tell a human something
happened. Building each of those once, generically, is what unblocks Phase 2
(e-sign), Phase 3 (payments), Phase 4 (screening), Phase 6 (helpdesk), and
Phase 7 (real data providers) — see `docs/ROADMAP.md`.

None of this is greenfield invention. The platform already has one real,
working example of "call an external thing safely, durably, and repeatably" —
the [property enrichment engine](./PROPERTY_DATA.md) — and one real example of
"store a secret safely" — PII field encryption. Phase 1 **generalizes both**
into substrate the rest of the product can share, rather than inventing new
patterns.

---

## Why the enrichment engine is the reference

`api/src/enrichment/` (see `docs/PROPERTY_DATA.md`) already proves the shape
every future integration should take:

| Property today | Where it lives | What Phase 1 generalizes it into |
|---|---|---|
| A consistent shape behind every external data source | `enrichment/source.rs`'s `Source` enum + a hand-written `match` in `enrichment/runner.rs::run_source` dispatching to one `run_<source>` function per variant | A real `Provider` **trait** any integration implements (#16) — see note below |
| One **live** implementation, the rest **deterministic simulations** | `enrichment/geocode.rs` vs `enrichment/simulated.rs` | Every new provider ships with a `simulate()` path from day one — sandbox-first is a rule, not a convention (#16) |
| Durable retrying execution | `api/src/scheduler.rs` (`JobOutcome::retry`/`failed`, `max_attempts`) | Unchanged — every Phase 1 piece rides this queue as-is, no new retry mechanism |
| Persist result + observable trail | `enrichment/runner.rs` + `enrichment.rs::record_run` → `enrichment_run` | A generic runner (#16) that persists a `Provider` call's result and writes an audit event |
| One function to swap simulated → real | `docs/PROPERTY_DATA.md`: "Replacing a simulated source with a real API … is a one-function change" | The same property, generalized: adding Stripe (#35), Plaid (#36), an ESP (#62), or a screening provider (#8) should each be a small file implementing `Provider`, not a new integration pattern |

> **Correction worth stating plainly:** there is no `trait` anywhere in
> `enrichment/` today (`grep -rn trait backend/crates/api/src/enrichment/`
> returns nothing). The "pluggable provider" language in `ARCHITECTURE.md` and
> `docs/PROPERTY_DATA.md` describes a *structural convention* — every provider
> function has the shape `async fn(...) -> Result<Data, EnrichmentError>`, and
> `runner::run_source`'s `match` is the only place one is plugged in — not
> literal trait-object polymorphism. #16 is where the platform gets its
> **first real `Provider` trait**; the enrichment engine is the proof that the
> *shape* works, not a trait to extract.

The takeaway: **Phase 1 has almost no new architecture to invent.** It's the
enrichment engine's shape, formalized into an actual trait and pulled out into
something reusable, plus three new pieces of storage (secrets, documents,
notifications) that follow equally-established patterns elsewhere in the
codebase (see below).

---

## 1. Secrets / KMS (#15)

**Extends:** `api/src/pii.rs`'s AES-256-GCM sealing (`encrypt`/`decrypt`/`last4`),
already used to encrypt SSN/gov-ID fields under a 32-byte key resolved by
`config.rs::pii_key_from_env`.

**Does not reuse `PII_ENC_KEY`.** A leaked provider credential and a leaked SSN
are different blast radii; they get independently rotatable keys. A new
`SECRETS_ENC_KEY` env var is resolved the same way, with one deliberate
difference: `pii_key_from_env` today falls back to deriving a key from
`JWT_SECRET` when unset (fine for dev ergonomics, and already flagged as a
prod risk by [#24](../../../issues/24)/[#25](../../../issues/25)). The
secrets key should **fail closed in prod from the start** — this issue
shouldn't have to be revisited by a hardening pass the way PII/JWT secrets are.

```
secret
  id            uuid
  tenant_id     uuid null   -- null = platform-wide (e.g. a shared sandbox ESP key)
  key           text        -- "stripe.api_key", "twilio.auth_token", …
  ciphertext    text
  nonce         text
  last4         text        -- masked display, same UX as PII fields
  created_by    uuid
  rotated_at    timestamptz
  created_at    timestamptz
```

Reveal is a server-side-only call (`secrets::reveal(db, tenant_id, key) ->
String`) consumed by provider clients — never serialized into an HTTP
response. The settings UI only ever shows `key: ****1234`, mirroring how
`profile:read_pii` gates PII decryption today.

**Audit:** `secret.set` / `secret.rotate` / `secret.delete`, key name only in
`metadata`, exactly the "log the fact, not the value" rule `docs/AUDIT.md`
already applies to `pii.reveal` and `profile.write`.

---

## 2. Outbound provider + webhook framework (#16)

**Outbound.** A `Provider` trait — the first real one in the codebase, since
(as noted above) `enrichment/` only proves the shape by convention: a typed
request/response pair, an error type (mirroring `EnrichmentError(pub
String)`'s single-variant newtype), and both a `call()` (the real integration)
and a `simulate()` (the CI/demo path) method. A generic runner — the
`Provider`-trait analogue of `enrichment/runner.rs::run_source` — calls the
provider, persists the result, and maps its error into `JobOutcome::retry(...)`
(transient) or `JobOutcome::failed(...)` (permanent), reusing `modules::JobOutcome`
exactly as it exists today; `enrichment.rs::backoff()` (`4 * 2^attempts`,
clamped) is the one existing exponential-backoff implementation worth reusing
verbatim rather than re-deriving. Credentials come from `secrets::reveal`
(#15); HTTP goes through `reqwest` (already a dependency, already used by
`enrichment/geocode.rs`).

**Inbound.** A single `POST /webhooks/{provider}` route. The closest existing
precedent is `tokens/principal.rs`'s `ApiPrincipal` guard — but it's a
**shared-secret bearer-token** pattern (hash the presented value, look up a
matching row by hash) with no HMAC-over-payload and no constant-time
comparison anywhere in the codebase (`grep`-confirmed: no `subtle`/`ConstantTimeEq`
crate is a dependency today). Verifying a signed webhook body needs a
genuinely new construct — an HMAC crate + a raw-body-capture request guard,
not a generalization of `ApiPrincipal`. A verified webhook does **not** get
handled synchronously in the request: it resolves a tenant and **enqueues a
`background_job`** for the owning module, so inbound events get the same
durability, retry, and audit trail as everything else instead of being a
special, un-retried code path.

This is the inbound half of a pair; [#68](../../../issues/68) is the outbound
half — vendors *subscribing* to Acre's own events via signed callbacks, using
the same queue-backed delivery + backoff + dead-letter shape.

---

## 3. Object storage + document service (#17)

Today the only document-shaped data in the platform is the lease agreement,
rendered as a string by `api/src/leasedoc.rs` and stored inline on
`lease_document`. Everything after Phase 1 needs general file storage: signed
lease PDFs (#6), rehab progress photos and lien waivers (#40), due-diligence
data rooms (#42), property media (#11), generated notices (#50).

```
document
  id                    uuid
  tenant_id             uuid
  owner_type            text   -- "property" | "lease" | "application" | "entity" | "deal" | …
  owner_id              uuid
  filename              text
  mime_type             text
  size_bytes            bigint
  checksum              text   -- sha-256, via the same hashing primitive as auth::hash_secret
  version               int
  previous_version_id   uuid null   -- re-upload creates a new row, not an overwrite
  retention_expires_at  timestamptz null
  created_by            uuid
  created_at            timestamptz
```

Storage is an S3-compatible object store (S3 / R2 / MinIO for local dev) — the
**first new external dependency** this epic needs (no object-storage crate
exists in `Cargo.toml` today), which makes it a natural first candidate for
the review process [#65](../../../issues/65) is building. Files are opaque
blobs keyed by `document.id`; the row is the only source of truth for
metadata. Access is always via a short-lived, permission-checked **signed
URL** — the API returns a URL, the Rocket process never proxies file bytes.

**Audit:** `document.upload` / `document.download` (the fact of access, not
the bytes) / `document.delete`.

---

## 4. Notifications (#18)

**This one is partially built already** — not from scratch. `auto_email` is a
real job kind today, owned by the `leasing` module
(`modules/leasing.rs:27`), enqueued from two call sites with a
`{ "template": "application_approved", "to": <email> }` payload
(`routes/applications/mod.rs:70-77`, `routes/public/apply.rs:90-97`). But
`handle_job`'s `("auto_email", _)` arm (`modules/leasing.rs:68-71`) is a
**fire-and-complete stub**: it marks the job `completed` with `{"sent": true}`
and does nothing else — no template is rendered, no provider is called, no
recipient ever receives anything, nothing is persisted.

This issue makes that job kind real without breaking its existing payload
contract:

```
notification
  id                  uuid
  tenant_id           uuid
  channel             text   -- "email" | "sms"
  template_key        text
  recipient           text
  status              text   -- "queued" | "sent" | "failed"
  provider_message_id text null
  body                text   -- the rendered output, for the audit/history trail
  background_job_id   uuid
  created_at          timestamptz
```

**Templating** reuses `leasedoc.rs::interpolate`'s `{placeholder}`
substitution — already proven against `theme.legal_templates` — rather than
adding a templating crate. A `notification_templates` JSON column on `theme`
(sibling to `legal_templates`) holds tenant overrides with a platform default.

**Provider** rides #16's `Provider` trait: a `simulate()` implementation
(logs + persists as `sent`, used in dev/CI and by every existing test/demo
today) and a slot for the real ESP/SMS provider ([#62](../../../issues/62))
to plug in later without touching call sites.

**Open design question, worth deciding before writing code:** should
`auto_email`/a new `auto_sms` stay owned by `leasing` (smallest diff — the job
kind and its two call sites don't move), or move to the new `integrations`
module (#19) so any future module can trigger a notification without a
dependency on `leasing`? Recommendation: move ownership to `integrations` —
reminders (#54), renewal notices (#49), and statutory notices (#50) all need
to send email and have nothing to do with leasing — but keep the payload
shape (`template` + `to`) unchanged so the two existing call sites don't need
to change, just which module's `handle_job` answers to the job kind.

---

## 5. Wiring it together: the `integrations` module (#19)

None of the above fits inside an existing module — they're cross-cutting
infrastructure, not one feature area — so they land behind one new module,
following `docs/MODULES.md`'s two-step recipe exactly:

1. `modules/integrations.rs` — a unit struct implementing `PlatformModule`.
   `manifest()` declares the `integrations` key, an `integrations:manage`
   permission, and the job kinds from #16/#18 (`auto_email`, `auto_sms`, and
   whatever webhook-delivery job kind #16 introduces).
2. One line added to `modules::registry()`.

Everything else — mounting routes, scheduling jobs, the `/modules`
enable/disable API, per-tenant `tenant_module` overrides — is already generic
and picks the new module up automatically (see `docs/MODULES.md`).

Unlike `flips` (`docs/MODULES.md`'s reference module, shipped as an opt-in
**preview**), `integrations` ships **on by default** — it's foundational
plumbing every tenant needs, not an optional feature.

**Routes** — `routes/integrations/`, one handler per file + a shared `dto.rs`,
matching every other route group (`ARCHITECTURE.md`'s `routes/*` convention):
secrets CRUD (write/rotate/delete only — plaintext is never returned),
document upload/list/download/delete, notification log listing.

**Frontend** — `frontend/src/app/console/integrations/`, reusing
`components/ui` and registered in `frontend/src/modules/registry.ts` with its
nav entry gated by `integrations:manage`.

---

## Definition of Done (recap)

From the epic (#5): upload/download a versioned document attached to any
entity; a sandbox webhook round-trips through the queue; a templated
email/SMS sends in a test; secrets are stored encrypted and consumed by a
provider client, never returned in a response.

## Sequencing

`#15` (secrets) has no dependents within this epic but is a prerequisite for
`#16`'s provider clients reading credentials. `#16` (provider + webhook
framework) and `#17` (documents) can build in parallel — neither depends on
the other. `#18` (notifications) depends on `#16` for its `Provider`
abstraction. `#19` (the `integrations` module) is the thin final layer that
gives `#15`–`#18` a tenant-facing home, so it's naturally last, even though
it's the smallest piece of code.
