# Notifications

Templated, multi-channel messaging: transactional email and SMS, browser
**Web Push**, **chat** (Slack/Discord), and the per-user **in-app inbox** —
with delivery providers the tenant configures from the console. Built on the
integration substrate ([`INTEGRATIONS.md`](INTEGRATIONS.md)): credentials in
the vault, delivery on the durable job queue, everything audited.

## Channels

| Channel | Job kind | Delivered by |
| --- | --- | --- |
| `email` | `auto_email` | the tenant's configured provider (**Resend**, **SendGrid**, **Postmark**) or the simulated fallback |
| `sms` | `auto_sms` | **Twilio** or the simulated fallback |
| `push` | `auto_push` | Web Push (VAPID) to every browser subscription the user holds |
| `chat` | `auto_chat` | **Slack** / **Discord** incoming webhook; skipped when none is configured |
| `in_app` | — | written directly to `notification`; `user_id` + `read_at` power the console inbox and the header bell |

All job payloads keep the original `{ "template": …, "to": … }` contract;
push adds `user_id`, and an explicit `provider_id` (used by the console's
per-provider **Test** button) overrides the channel's default provider.

## Delivery providers (end-user configurable)

`notification_provider` rows hold a tenant's configured services — channel,
kind, non-secret `config` (from address, account SID), and a `secret_ref`
pointing at the vaulted credential (API key, auth token, or webhook URL —
never stored or returned in plaintext; the UI shows `last4`). One provider
per channel is the default; channels without a provider fall back to the
deterministic simulated sender.

Two gates decide whether a real network call happens:

1. **Tenant config** — which provider a channel routes through.
2. **`LIVE_PROVIDERS`** (platform env) — the channels allowed to leave the
   box (`email,sms,push,chat` or `all`). Everything else simulates, which is
   what dev/CI exercise. A live channel with no provider fails loudly rather
   than silently dropping mail.

Console: **`/console/notifications` → Delivery providers** (gated by
`integrations:manage`): add, rotate credential, enable/disable, make default,
delete, and **Test** (queues a `test_notification` through that provider).

Routes: `GET/POST /integrations/providers`,
`PATCH/DELETE /integrations/providers/<id>`,
`POST /integrations/providers/<id>/test`.

## Web Push

Standards-based, no SDK:

- **RFC 8291** payload encryption (`aes128gcm`: P-256 ECDH + HKDF-SHA256 +
  AES-128-GCM), unit-tested against the RFC's Appendix A vector byte for
  byte.
- **RFC 8292 (VAPID)**: an ES256 JWT over the push service's origin. The
  platform keypair is generated on first use and kept in the secrets vault
  (`webpush.vapid_private_key`) — no key ceremony; set `VAPID_SUBJECT` to a
  `mailto:` you own for production.
- Subscriptions live in `push_subscription` (endpoint + client keys, one row
  per browser); the push service answering 404/410 prunes the row
  automatically.

Frontend: `frontend/public/sw.js` (service worker) + `src/lib/push.ts`
(permission → register → subscribe with the VAPID key → sync to the
backend). The console's **Browser push** card toggles it per device.

Routes: `GET /notifications/vapid_key`,
`POST/DELETE /notifications/push_subscriptions`,
`POST /notifications/test_push`.

## In-app inbox

User-directed notifications (`channel = in_app`) are written synchronously —
the row *is* the delivery — and surface in two places:

- the **bell** in the console header (unread badge via
  `GET /notifications/unread_count`), and
- **`/console/notifications`** (list, mark read, mark all read).

Routes: `GET /notifications/inbox`, `POST /notifications/<id>/read`,
`POST /notifications/read_all`. Rows are always scoped to the signed-in
user; no extra permission is needed to read your own inbox.

## Event fan-out

`notify::notify_staff(db, tenant, permission, template, vars, owner,
trigger)` fans one workspace event out to every active member holding a
permission: an in-app entry each (immediate), an `auto_push` job each
(per-user retry isolation), and one `auto_chat` message when a chat provider
is configured. Fan-outs audit once as `notification.broadcast` with the
recipient count.

First wired event: **application submitted** → everyone with
`application:read`. Renewals, reminders, and maintenance events ride the
same helper as they land.

## Templates

The `{placeholder}` engine from lease documents renders every channel:
platform defaults live in `api/src/notify/mod.rs` (`application_approved`,
`application_received`, `application_submitted`, `test_notification`), and
tenants override per key via `theme.notification_templates` — a body string,
or `{ "subject": …, "body": …, "sms": … }` merged field by field. Email uses
`subject` + `body`; SMS and chat use the short `sms` text; push and in-app
use `subject` as the title with the `sms` text as the body.

## Idempotency & audit

Payloads carrying owner context get a natural key
(`channel:template:owner_type:owner_id:trigger[:user_id]`, unique per
tenant): retried jobs and duplicate triggers can't double-send, and
re-submitting the same event can't re-notify the same user. Every send
audits as `notification.send` (template + channel + status, never the
rendered body); provider CRUD audits as `notification_provider.*`; push
subscriptions as `push.subscribe` / `push.unsubscribe`.
