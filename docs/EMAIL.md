# Email Integration

The rest of a real email story (issue #62), on top of the transactional
outbound sends in [`NOTIFICATIONS.md`](NOTIFICATIONS.md): inbound email
routed into the product, per-tenant inbound addresses, white-label
deliverability (SPF/DKIM/DMARC) for custom domains, and a logged
communication history.

## Outbound (recap)

Templated sends already ride the provider framework: a tenant connects
**Resend / SendGrid / Postmark** from the console (`notification_provider` +
vaulted credential), `LIVE_PROVIDERS=email` flips the channel live, and
everything else simulates — see [`NOTIFICATIONS.md`](NOTIFICATIONS.md).
Every outbound send is logged in `notification` (rendered subject/body,
status, provider message id).

## Inbound email → ticket / lead

Tenants point their ESP's inbound hook (Postmark inbound, SendGrid Inbound
Parse, SES receiving) at the platform's signature-verified webhook door:

```
POST /webhooks/inbound_email?tenant=<slug>
X-Acre-Signature: sha256=<hmac of raw body>     # secret: webhook.inbound_email.secret
{ "from": …, "to": …, "subject": …, "text": … } # Postmark-style From/To/Subject/TextBody also accepted
```

The verified event rides the durable queue (`webhook_event`) and routes by
the **to-address** local part (`api/src/mail.rs`):

| Address | Routed to |
| --- | --- |
| `ticket+<uuid>@<slug>.<domain>` | A comment on that maintenance ticket's timeline (author `None`, "Email reply from …") |
| `leasing@<slug>.<domain>` | A CRM **lead** — created on first contact (staff holding `application:read` are notified via `lead_received`), refreshed with the latest message on repeat contact; a closed lead reopens |
| anything else | Logged unmatched |

Per-tenant addresses hang off `INBOUND_EMAIL_DOMAIN` (default
`in.acrenexus.com`): each ticket's reply-to address is surfaced on
`GET /tickets/<id>` (`inbound_email_address`), and the leasing inbox on
`GET /leads` (`inbox_address`).

**Comms logging** — every inbound message lands in `inbound_email` (from,
to, subject, body, where it routed) — the inbound half of the communication
history; outbound lives in `notification`. Read it at
`GET /integrations/inbound-emails` (`integrations:manage`); every receipt
audits as `email.inbound` (routing metadata, never the body).

## CRM leads (the #46 seed)

`lead` rows (`name`, `email`, `phone`, `source`, `status
new → contacted → toured → applied → closed`, `notes`, `last_message`) are
workable from **`/console/leads`**: `GET /leads` (`application:read`),
`PATCH /leads/<id>` (`application:write`). Inbound leasing email is the
first automated source; manual and website capture ride the same rows.

## White-label deliverability (SPF / DKIM / DMARC)

Branded mail from a tenant's **custom domain** shouldn't land in spam.
Mirroring the domain-verify flow, every custom `domain` surfaces the TXT
records to publish (`email_dns_records` on `GET /domains`):

| Record | Name | Value |
| --- | --- | --- |
| SPF | `<hostname>` | `v=spf1 include:spf.acrenexus.com ~all` |
| DKIM | `acre._domainkey.<hostname>` | `v=DKIM1; k=rsa; p=<per-tenant selector value>` |
| DMARC | `_dmarc.<hostname>` | `v=DMARC1; p=quarantine; rua=mailto:dmarc@acrenexus.com` |

`POST /domains/<id>/verify-email` (`domain:manage`) checks them through the
sandbox-first **DNS provider** (`providers/dns.rs`): simulated (every record
passes) unless `LIVE_PROVIDERS` lists `dns`, in which case TXT records are
resolved over **DNS-over-HTTPS** (Cloudflare `application/dns-json`) with no
resolver dependency. Per-record results land in `email_dns_status`,
`email_verified_at` is set once all three pass, and the check audits as
`domain.verify_email`. The console's Domains page fronts the records +
verify button per custom domain.

## Configuration

```bash
INBOUND_EMAIL_DOMAIN=in.acrenexus.com   # per-tenant inbound addresses hang off <slug>.<this>
LIVE_PROVIDERS=email,dns                # flip outbound email + real DNS checks live
# per tenant: webhook.inbound_email.secret in the credential vault
```

## Definition of Done (how to see it work)

A templated email sends through a configured ESP in sandbox; a signed
inbound POST replying to a ticket's `ticket+<id>@…` address appears on the
ticket timeline; mail to `leasing@…` creates a lead visible at
`/console/leads`; and a custom domain's SPF/DKIM/DMARC verify (simulated in
dev), flipping the domain to `email_verified`.
