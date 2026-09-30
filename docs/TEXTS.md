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
| `PATCH /texts/<id>` | `message:manage` | `{ status: "open" \| "done" }` |
| `POST /texts/simulate` | `message:manage` | test mode only: act as if `phone` texted `body` |
| `POST /webhooks/twilio/sms?tenant=<slug>` | Twilio signature | an inbound text |
| `POST /webhooks/twilio/status?tenant=<slug>` | Twilio signature | delivery status |

An inbound text notifies everyone with `message:read` (`text_received`: inbox +
push). Invite and password-reset texts are filed as "(sign-in link sent —
hidden)" so a staff member can't read someone else's sign-in link.

## Going live

1. Integrations → add a Twilio SMS provider (account SID, sending number, auth
   token in the vault).
2. `LIVE_PROVIDERS=sms` (or `all`) and `PUBLIC_API_URL` set to the public API
   address — the signature is computed over that exact URL.
3. In Twilio, point the number's "A message comes in" webhook and the status
   callback at the two URLs `GET /texts/status` shows.

A webhook without a valid `X-Twilio-Signature` for that workspace's auth token
is refused with 403.

## Still to come

Saved replies, assigning a conversation, linking an unknown number to a
resident, storing MMS photos on the resident / work order, "report this as a
maintenance request" from a text, missed-call text-back, and quiet hours for
non-urgent texts.
