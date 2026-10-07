# Vantedge × Solnyxus — Google Workspace integration notes

Status: **planned, not started.** The Solnyxus Hub (`daedalus-it` repo, https://solnyxus.com) is getting the full Google Workspace integration first; this note is what Vantedge will take from it, written from a read of this repo on 2026-10-07. Prove the Hub on the real server first, then port in the order at the bottom.

Design to copy lives in the Hub repo: `docs/google.md`, `docs/workspace.md`, `docs/drive.md`, `docs/chat.md`, `docs/mail.md`, `docs/seo.md`, `docs/atlas.md`.

Where Vantedge runs (from the Hub's `deploy/`): `vantedge.solnyxus.com`; API `127.0.0.1:8200` (`/api/` is mounted there), web `127.0.0.1:3200`, one-off migrate/boot port `18200`; units `vantedge-api`, `vantedge-web`; env `/etc/vantedge/{api,owner,web}.env`; `LIVE_PROVIDERS=email` is set (everything else simulates until listed).

## Facts that shape every item
* **Secrets vault** (`api/src/secrets.rs`): AES-256-GCM under `SECRETS_ENC_KEY`, keyed `(tenant_id, key)`, `reveal` falls back to the platform row (`tenant_id` NULL). Plaintext is never returned over HTTP. Per-workspace secrets are written by `PUT /integrations/secrets`; **there is no HTTP route to write a platform-wide secret** (only code does) — platform-level Google credentials need either seed SQL or a new platform-admin route. **Never rotate `SECRETS_ENC_KEY`.**
* **Providers** implement `Provider` (`call`/`simulate`, `providers/mod.rs:93`) and run through `providers::run`; `providers::is_live(key)` gates real calls on `LIVE_PROVIDERS`.
* **RLS and jobs:** requests run in a transaction with `SET LOCAL app.tenant_id`; the scheduler (`scheduler.rs`, every 3 s) uses the raw connection with no tenant GUC, so **background jobs are cross-tenant and must filter `tenant_id` explicitly**. New recurring jobs follow `ensure_recurring_jobs` (`main.rs:183-191`) and route through `modules/*.rs` (a disabled module parks its jobs). Runtime role is `acre_app` (no BYPASSRLS).
* "Tenant" in this codebase means **workspace**, not renter.

## 1. Email through Google — needs an `smtp` provider
* Email providers live in `api/src/notify/delivery.rs` (`EmailDelivery`: `resend`, `sendgrid`, `postmark`; no SMTP, no `lettre`). Per-workspace.
* Plan: add `"smtp"` to `PROVIDER_CHANNELS` (`notify/mod.rs:48`; `validate_channel_kind` and a unit test read it), an `smtp` arm in `EmailDelivery::call` using **lettre** (add to workspace deps + `api/Cargo.toml`), config `from/host/port/username` in `notification_provider.config`, the app password from the vault via `row.secret_ref` (`provider_secret_ref(id)`, written by `create_provider.rs`/`update_provider.rs`), the kind in the console form (`frontend/src/app/(console)/console/notifications/providers.tsx`), and the doc strings (`routes/integrations/dto.rs`, `modules/integrations.rs`). No migration (`kind` is a free string).
* Relay mode: `smtp-relay.gmail.com:587` STARTTLS with **no credentials** (server IP allowlisted in the Google Admin console) — make auth optional in the provider.
* Surprise: `MessageRequest` is only `{to, subject, body}` — no HTML, attachments, reply-to, cc. Extend it before sending PDFs or invites by email.

## 2. Calendar — appointments exist, no Google link
* Appointments (`entity/appointment.rs`, `appointments.rs`, `routes/appointments.rs`): kinds repair/showing/inspection/other; staff offer ≤ 4 windows; `confirm()` (`appointments.rs:374`) sets `starts_at/ends_at/status=confirmed`; a 15-minute `appointment_reminders` job. Inspections have only a date; maintenance tickets only `due_date`.
* Plan: per-workspace Google connection (refresh token as a vault secret, per connection — the Hub's `google_connections` model is the template), an `appointment.google_event_id` column (migration), push on `confirm`/`update_appointment`/`decline`, `calendar.events` (+ `calendar.app.created` for a dedicated calendar), hash-skip unchanged events, only touch events carrying our key. Add `.ics` (none exists) for tenants/vendors who don't use Google.

## 3. Contacts — no contact entity
* Sources: residents (`user`, `user_profile`, `resident_profile`), owners (`owner`), vendors (`counterparty`), leads (`lead`).
* Plan (Hub design): push only to a contact group per workspace/kind, link table + hash, delete only what the app created, import never automatic. Job runs cross-tenant → filter by tenant explicitly.

## 4. Drive export + sharing
* Documents: `entity/document.rs` (polymorphic `owner_type/owner_id`, versioned); storage `storage.rs` (local or S3); downloads are signed URLs. PDFs are hand-rolled: `pdf.rs::text_to_pdf` (leases, deposits, payouts, screening), `pdfdoc.rs::render` (reports, owner statements). Call sites: `routes/reports/mod.rs:108`, `routes/backoffice/reports.rs`, `routes/crm/mod.rs:1231`. No Drive client; `reqwest` (rustls) is the HTTP client.
* Plan (Hub design): `drive.file` only; folders `Vantedge / <workspace> / <kind>`; audit table; **sharing locked down** to the document's own parties (the lease's resident/owner), staff, or allow-listed domains; reader/commenter only; ≤ 20 recipients; share after upload succeeds. Read bytes via `ObjectStore::get_bytes` (local backend only today — S3 needs the signed-URL path).

## 5. Chat
* `ChatDelivery` (slack/discord incoming webhooks) already serves staff notifications; add a `google_chat` kind for *outbound staff alerts* (payload `{"text": …}`) — trivial.
* Public live chat is greenfield: `message_thread` is lease-bound/portal-only; public intake today is `/public/tour-requests`, `/public/applications`, inbound email → lead. Build the Hub's design (Chat app + service account, polling, verified events) with abuse controls; offline → a lead. The public site must exist first (see 7).

## 6. Sign in with Google — exists, **harden before enabling**
* Present: `api/src/oauth.rs` (Google/Microsoft/Apple, PKCE, signed state), `routes/auth/oauth.rs`, `entity/federated_identity.rs`, login/callback pages. Config: vault keys `oauth.google.client_id|client_secret` (platform-wide), `LIVE_PROVIDERS` must include `google`, redirect `${PUBLIC_APP_URL}/auth/callback`.
* **Problems to fix first** (`oauth.rs:433-441`, `complete()`): the ID token's **signature, `aud`, `exp` are not verified**, `email_verified` and `hd` are not checked, it **auto-links to an existing user by email including staff**, and it **auto-creates a renter account for any Google user** (open signup). Port the Hub's verified ID-token code (RS256 against Google's JWKS, `iss/aud/exp/iat/nonce`, `email_verified`, `hd`) and an explicit allow-list/domain rule; decide whether open signup is wanted at all.
* No route writes the two platform secrets — seed them or add a platform-admin route.

## 7. SEO — the public site is not currently built
* Exists: `frontend/src/lib/seo.ts`, `seo-schema.ts` (JSON-LD, tested), `business_profile.google_site_verification`, console page `integrations/website.tsx`, `/public/listings|theme|resolve`, noindex headers.
* **Stale docs:** `app/(gate)/page.tsx` redirects `/` to `/console`; `app/sitemap.ts`, `robots.ts` and listing pages were deleted in commit `b07dd59` ("rebuilding the frontend") yet `docs/SEO.md` and `website.tsx` still describe them. Decide whether the public site comes back; if so restore sitemap/robots first, then GA4 behind a consent banner (`docs/SEO.md` already says consent first).

## 8. Registering Vantedge in the Hub's Atlas
* The Hub's production seed (`deploy/seed/atlas.sql`) records hostname, ports, units and flows. Keep it in step (note `VT_BOOT_PORT` 18200 is not in the Hub's `PORT_VARS`). Nothing to change here.

## Suggested order
1. `smtp` provider (+ optional auth) → 2. **fix and enable Google sign-in** (verify the ID token; no open signup) → 3. Drive export → 4. Calendar for appointments (+ `.ics`) → 5. Contacts → 6. SEO once the public site is back → 7. Chat last.

## Before any of it
* Platform-wide Google credentials need a write path (item 6). Register redirect URIs per feature in the Google Cloud console. Nothing in the Hub's Google code has run against real Google yet — port what survives there.
* Backend checks before pushing: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`; integration tests with `TEST_DATABASE_URL=postgres://localhost:5432/acre_it cargo test -p api itest`; frontend `npm run lint && npm run format:check && npm run typecheck && npm run test && npm run build`.
