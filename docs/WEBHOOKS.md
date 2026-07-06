# Vendor API Outbound Webhooks

"Subscribe, don't poll" (issue #68) — the outbound counterpart of the
inbound webhook-ingestion framework in [`INTEGRATIONS.md`](INTEGRATIONS.md).
The scoped vendor token API (`/api/v1`) lets integrators read; webhook
subscriptions push them a **signed callback** when something they care about
changes.

## Subscriptions

A vendor registers with the same API key that gates its reads:

```
POST /api/v1/webhooks
{ "url": "https://vendor.example/hooks/acre",
  "event_types": ["listing.updated", "payment.recorded"],
  "description": "sync worker" }
```

- **Scope-gated** — each event type maps to the permission that gates the
  matching read (`GET /api/v1/webhooks/events` lists the catalog). A token
  can never subscribe to data it couldn't already read; revoking or
  narrowing a token silences its subscriptions at emission time.
- **Tenant-scoped** — subscriptions belong to the token's tenant and are
  visible only to the token that created them.
- The response carries the **signing secret exactly once**
  (`whsec_…`, vaulted under `webhook_sub.<id>.secret`), like an API token's
  raw value.
- `GET /api/v1/webhooks`, `PATCH /api/v1/webhooks/<id>` (url / event types
  re-validated / enable / disable), `DELETE /api/v1/webhooks/<id>`.

### Event catalog

| Event | Required scope | Emitted when |
| --- | --- | --- |
| `listing.created` | `listing:read` | A listing is created from the console |
| `listing.updated` | `listing:read` | A listing is edited from the console |
| `application.created` | `application:read` | An application is submitted (any intake door) |
| `payment.recorded` | `payment:read` | A payment settles as paid |
| `maintenance_ticket.created` | `maintenance:read` | A work order is opened |

## Delivery

Emission fans out onto the retrying job queue (`webhook_deliver`, owned by
the `vendor_api` module): one `webhook_delivery` row per subscriber per
event, then a POST of

```json
{ "id": "<delivery id>", "event": "listing.updated",
  "created_at": "…", "data": { … } }
```

with headers `X-Acre-Signature: sha256=<hex>` (HMAC-SHA256 of the raw body
under the subscription secret — the **same scheme** as inbound ingestion, so
`providers::webhook::verify` is the reference implementation),
`X-Acre-Event`, and `X-Acre-Delivery`.

- **At-least-once** with the platform's shared exponential backoff; after
  `max_attempts` the delivery **dead-letters** (`status = dead`) with its
  last error on display.
- **Sandbox-first** — deliveries simulate (succeed deterministically)
  unless `LIVE_PROVIDERS` lists `webhooks`; a simulated URL containing
  `fail` refuses delivery, so the retry → dead-letter path is demoable
  offline. Every attempt audits as `provider.call`.

## Observability & replay

- `GET /api/v1/webhooks/<id>/deliveries` — per-subscription history, newest
  first: status (`pending | delivered | dead`), attempts, the subscriber's
  HTTP status, the last error, timestamps.
- `POST /api/v1/webhooks/<id>/deliveries/<delivery_id>/replay` — re-send as
  a **fresh delivery row** (history stays honest), audited as
  `webhook_delivery.replay`.

## Definition of Done (how to see it work)

Mint a token with `listing:read`, register a webhook for `listing.updated`,
edit a listing in the console → a signed, verifiable callback arrives
(simulated delivery in dev). Point a second subscription at a URL containing
`fail` → deliveries retry with backoff and dead-letter, with the whole
history visible (and replayable) from the vendor API.
