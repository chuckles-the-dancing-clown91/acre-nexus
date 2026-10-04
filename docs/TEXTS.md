# Two-way texts

Vantedge roadmap phase 2 — the text inbox Alpha Power Wash has, for rentals.
Residents text the workspace's number; the office answers from **Console →
Texts**; every automatic text (receipts, reminders, e-sign links) lands in the
same conversation.

## Model

| Table | What it holds |
| --- | --- |
| `sms_thread` | One conversation per phone number (E.164) per workspace: matched lease + resident name, `open`/`done`, unread count, last preview, and `opted_out_at` |
| `sms_message` | The timeline: `in` / `out`, body, `received`/`queued`/`sent`/`failed`/`blocked`, template key for automatic texts, who typed it, MMS count |

Both are tenant-owned with enforced RLS. A new number is matched to the lease
whose `tenant_phone` normalises to it (an active lease wins).

## STOP always wins

- **Opt out:** STOP, STOPALL, UNSUBSCRIBE, CANCEL, END, QUIT, OPTOUT, REVOKE and
  the Spanish ALTO, PARAR, BAJA — only when that is the whole message.
- **Opt back in:** START, UNSTOP, SUBSCRIBE, OPTIN — and YES/SÍ, but only from a
  number that had stopped.
- While a number is opted out, a console reply is refused (409) and every
  notification job to it completes as `{ "skipped": true, "reason": "opted_out" }`.
- Twilio sends its own STOP/HELP confirmations, so the webhook answers with
  empty TwiML.

## API

| Route | Permission | |
| --- | --- | --- |
| `GET /texts/status` | `message:read` | test mode or live, and the webhook URLs to paste into Twilio |
| `GET /texts?status=open\|done` | `message:read` | conversations, newest first (max 200) |
| `GET /texts/<id>` | `message:read` | one conversation; marks it read |
| `POST /texts/<id>/reply` | `message:manage` | `{ body }` — queued, then marked sent/failed/blocked |
| `POST /texts` | `message:manage` | `{ phone, body }` — text a new number |
| `PATCH /texts/<id>` | `message:manage` | `{ status: "open" \| "done", assignee, marketing_consent, link: { kind: "resident" \| "lead" \| "vendor" \| "none", id }, display_name }` |
| `POST /texts/simulate` | `message:manage` | test mode only: act as if `phone` texted `body` |
| `POST /texts/simulate-call` | `message:manage` | test mode only: act as if `phone` called and nobody answered |
| `POST /webhooks/twilio/sms?tenant=<slug>` | Twilio signature | an inbound text |
| `POST /webhooks/twilio/status?tenant=<slug>` | Twilio signature | delivery status |
| `POST /webhooks/twilio/voice?tenant=<slug>` | Twilio signature | a call to the texting number |
| `POST /webhooks/twilio/voice/after?tenant=<slug>` | Twilio signature | whether the rung phone answered |

An inbound text notifies everyone with `message:read` (`text_received`: inbox +
push). Invite and password-reset texts are filed as "(sign-in link sent —
hidden)" so a staff member can't read someone else's sign-in link.

## Whose number is it

A new conversation is matched to a resident by the lease's phone, else to the
newest prospect (lead) with that number, else to a vendor or other
counterparty; the match names the conversation and sets `lease_id`,
`lead_id` or `counterparty_id`. Staff can say whose it is by hand with
`PATCH /texts/<id>` and `link` (a lease, a lead, a counterparty, or `none` to
clear), and rename it with `display_name`. Both are audited
(`sms.thread_update`). Migration 074 adds the two link columns.

## Missed calls

Point the Twilio number's "A call comes in" webhook at the voice URL from
`GET /texts/status`. The caller hears `texts.voice_greeting` ("Thanks for
calling {company}."). When `texts.forward_number` is set the call rings that
phone for 20 seconds; otherwise, or when nobody answers (anything but a
completed dial), the call is a **missed call**: it is filed in the caller's
conversation as "Missed call", the conversation opens and counts as unread,
everyone with `message:read` hears (`text_missed_call`), and the caller is
texted `texts.missed_call_reply` (with `{company}` filled in) so the
conversation carries on in the inbox. The text-back goes at any hour (it
answers something the person just did), at most once per number every
`texts.missed_call_hours` (4) through the notice log, never to a number that
texted STOP, and not at all with `texts.missed_call_reply_on` off. The
TwiML is built by `voice_twiml` and unit-tested.

## Going live

1. Integrations → add a Twilio SMS provider (account SID, sending number, auth
   token in the vault).
2. `LIVE_PROVIDERS=sms` (or `all`) and `PUBLIC_API_URL` set to the public API
   address — the signature is computed over that exact URL.
3. In Twilio, point the number's "A message comes in" webhook, the status
   callback, and "A call comes in" at the three URLs `GET /texts/status`
   shows.

A webhook without a valid `X-Twilio-Signature` for that workspace's auth token
is refused with 403.

## Still to come

Nothing from round two. Saved replies, assigning, MMS filing, repair links,
ratings by text and quiet hours are described in
[`FIX-PLAN.md`](FIX-PLAN.md) (F14 to F17); linking a number and missed-call
text-back are above.
