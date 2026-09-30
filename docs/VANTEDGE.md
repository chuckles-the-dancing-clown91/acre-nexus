# Vantedge — rebrand & build-out roadmap

**Vantedge** (vantage + edge) — *See every angle. Stay a step ahead.*

Short form for tight spaces (tab titles, texts, the login card): **"See every angle."**
Other lines considered: "Your portfolio, from the high ground." · "The view that
pays." · "Every door. Every dollar. One view."

Acre Nexus is now Vantedge. This document is the plan for giving it the same
build-out Alpha Power Wash got — the features that made Alpha feel finished to
the people using it every day (two-way texts, real logins for clients, reminders
that run themselves) — plus the property-management ideas that fit the
partnership letter (Bree's company as customer zero, Alpha as the first vendor,
the Foundation as the second portfolio).

Legend: ☐ planned · ◐ in progress · ☑ done.

---

## What each product has today

| Area | Alpha Power Wash | Acre Nexus → Vantedge |
| --- | --- | --- |
| Texts | Two-way inbox, Twilio webhooks (signed), STOP/START incl. Spanish, saved replies, assign, MMS, missed-call text-back, quiet hours, consent | **Outbound only** (Twilio send). No inbound, no inbox, no STOP handling |
| Client logins | Invite → set password (7-day link), forgot/reset (24 h link), throttled login, TOTP + recovery codes, passkeys | Password login + TOTP + Google/Microsoft/Apple. **Invited users get a random password and can never sign in**; no forgot/reset |
| Portal | Client portal: book, pay, proposals, documents, people on an account | Resident portal: lease, pay, maintenance, messages, applications. **No owner portal, no vendor portal** |
| Scheduler | ~30 jobs retimed from Settings → Schedule, run history | Durable job queue + reminders engine |
| Reviews / marketing | Google reviews, review asks, offers, campaigns, referrals | none |
| Spanish | Emails, texts, PDFs, crew app | none |
| Offline / push | Installable PWA, Web Push | Web Push |
| Audit / backups | Append-only audit, nightly encrypted backups | Full domain audit; backups via deploy |

Vantedge already goes much further than Alpha on accounting, trust, screening,
e-sign, deals and rehab. The build-out is about the **people-facing layer**.

---

## Phase 0 — Rebrand ◐

- ☑ User-facing name, page titles, login and landing copy, console chrome,
  notification fallback company name, README and docs headers → **Vantedge**.
- ☑ Slogan on the login and set-password pages, and a "Powered by Vantedge"
  footer on each tenant's public listings site (which stays white-label).
- ☑ New mark and favicon (a rising chevron over a horizon — vantage point
  meets edge, `components/Brand.tsx`, `app/icon.svg`); accent moved from orange
  `#f5451f` to the Vantedge teal `#0e7c86` (dark `#3cc3c9`), warm neutrals
  kept. Tenants still override the accent with white-label branding.
- ☐ Internal identifiers (personas `acre_*`, DB roles `acre_app`, token prefix
  `acre_live_`, `X-Acre-Signature`, `acrenexus.example` seed domains). These are
  data and API contracts — rename behind a migration that accepts both old and
  new for one release (`vtg_live_` tokens, `X-Vantedge-Signature` sent alongside
  the old header), then drop the old names.
- ☐ Domain, email sender (`hello@vantedge…`), Twilio sender name, OAuth app
  names at Google/Microsoft/Apple — outside the code, needs Charles/Bree.

## Phase 1 — Real logins for everyone (client passwords) ◐

The single biggest blocker to Bree's company going live: residents, owners and
staff who are invited cannot sign in.

- ☑ **Set-your-password invite.** Inviting a member sends an email/text with a one-time link (7 days). Opening it sets the
  password and activates the account.
- ☑ **Forgot password / reset.** "Forgot your password?" on the login page →
  24-hour link by email (and text when the account has a phone). Reset
  invalidates old sessions. Never reveals whether an email exists.
- ☑ Links are single-use, stored only as a SHA-256 hash, a new link retires
  the old one, and every step is audited (`auth.password_reset_request`,
  `auth.password_reset`, `auth.invite_accept`, `auth.password_change`,
  `auth.login_link_send`). The endpoints share the tight auth rate-limit bucket.
- ☑ **Resend login link** button on Members (`POST /members/<id>/login-link`).
  ☐ the same on a lease's tenant.
- ☑ **Change password** on My profile (`POST /auth/password/change`).
- ☐ **Login throttling** — lock an email/IP pair for 15 min after 10 failures
  (the rate limiter already exists; add a failure counter).
- ☐ **Passkeys** (WebAuthn) as in Alpha — face/fingerprint sign-in on phones,
  which is how residents actually pay rent.
- ☐ **Recovery codes** for TOTP (Alpha has them; Vantedge doesn't).
- ☐ **Office MFA policy** — require two-step for staff roles.

## Phase 2 — Two-way texts ◐

Property management runs on texts. Residents text the office about a leak;
the office texts back from one shared inbox.

- ☑ **Inbound webhook** `POST /webhooks/twilio/sms` verifying Twilio's
  `X-Twilio-Signature` (HMAC-SHA1 over URL + sorted params, tested against
  Twilio's documented example); `POST /webhooks/twilio/status` records
  sent/failed. See [`TEXTS.md`](TEXTS.md).
- ☑ **Conversations**: one thread per phone number per workspace, matched to
  a resident (lease tenant phone) or member; unread counts; open / done.
- ☑ **STOP / START**: STOP, STOPALL, UNSUBSCRIBE, CANCEL, END, QUIT and the
  Spanish ALTO, PARAR, BAJA opt out; START, UNSTOP (and YES only from a
  stopped number) opt back in. STOP always wins: every outbound text checks the
  opt-out list first, including the notification templates.
- ☑ **Console inbox** (`/console/texts`): threads, reply, mark done, test-mode
  "pretend they texted back".
- ☐ Saved replies, assign a thread to a teammate, link an unknown number to a
  resident, MMS photos filed to the resident / work order.
- ☐ **Text → work order.** A resident texting "my sink is leaking" gets a
  one-tap "Report this as a maintenance request?" link.
- ☐ Missed-call text-back (Twilio voice) as in Alpha.
- ☐ Quiet hours for non-urgent texts (8 AM–9 PM local) and marketing consent.

## Phase 2B — The back office: one set of hours, everything adds up ◐

Built — see [`BACKOFFICE.md`](BACKOFFICE.md) for how it works.

Alpha's back office works because every number comes from the same records: a
time entry is logged *against a job*, and that one row is payroll, overtime,
labor cost, job profit and the crew report at once. Vantedge gets the same
spine, translated to property management — the "job" is a **work order**, a
**rehab project**, or a **property**, and the in-house hours don't just cost
money, they are **billed to the owner**.

```
 clock in on a work order ─┐
 miles / receipts / parts ─┼─► approved time + costs ─┬─► payroll week (CA daily OT) ─► Gusto / CSV / PDF
                           │                           ├─► work-order cost: pay + OT share + burden
                           │                           │     + parts + mileage + expenses + vendor bills
                           │                           ├─► bill to owner: hours × bill rate + parts
                           │                           │     + markup → AP bill on the owner's LLC
                           │                           │     → ledger → owner statement → payout
                           │                           └─► reports: profit by work order / property /
                           │                                 technician / category / month, tax package
```

**People (user management → HR)** ☑
- Employee profile on any staff member: title, employment type (full-time /
  part-time / seasonal / 1099 contractor), pay rate, **bill rate** (what an hour
  of their work is charged to owners), hire / end date, weekly target, default
  vehicle, mileage reimbursed, emergency contact, notes. Pay rates are behind
  `payroll:read`.
- Shifts (work / on call / training) and time off (vacation / sick / personal /
  unpaid → approve / deny; approved time off blocks the schedule).

**Time clock** ☑
- Clock in on a work order, rehab project, property, or travel / shop / office
  time; one open entry per person; overlapping entries refused; breaks.
- Pay rate and bill rate **frozen when the entry closes** — a raise never
  rewrites last month.
- Missed punches: an entry open past N hours (setting, default 12) is closed at
  the best evidence (the work order's resolved time, else start + 8h), flagged,
  and held from payroll until the office resolves it; the technician can say
  when they really finished.
- Approval by the office; approved entries are locked for the technician.
- Optional location stamp at clock-in/out only, flagged if farther than the
  radius from the property (never blocks).

**Overtime & payroll** ☑
- Alpha's engine, ported exactly: weekly FLSA (1.5× over 40) or **California
  daily** (1.5× over 8, 2× over 12, seventh consecutive day 1.5× first 8 / 2×
  after, daily OT not double-counted toward 40); contractors straight time.
- Payroll report by Monday–Sunday week: days, entries, hours, regular, OT 1.5×,
  DT 2×, rate, gross, mileage paid back. Approved-only option. CSV + PDF.
- Gusto: match people, push approved hours (Regular / Overtime / Double
  overtime) into an unprocessed payroll; excluded hours listed. Sandbox-first
  like every other provider.

**Expenses & mileage** ☑
- Expense: date, category (fuel, mileage, materials, equipment, repairs,
  vehicle, insurance, payroll, marketing, software, licenses, other), vendor,
  amount, deductible, company / own vehicle, reimbursable, **billable to owner**,
  tied to a work order / rehab / property / employee; receipts ride the
  document service.
- Mileage: miles × the mileage rate setting (IRS standard, default $0.70), with
  odometer start/end and round trip; own-vehicle miles reimbursed.

**Costing** ☑
- Per work order: labor pay + share of the week's OT premium (spread by hours)
  + labor burden % (not on contractors) + parts from inventory at cost +
  mileage + expenses + outside vendor bills → total cost; billed-to-owner is the
  revenue; gross and gross %; optional overhead per labor hour for net.
- **Bill to owner**: approved hours × frozen bill rate, parts, billable
  expenses and a markup % → one AP bill from "In-house maintenance" on the
  property's LLC, through the existing approve → post → pay flow, so it lands
  on the owner statement and comes out of the payout. Printable as a PDF.
- Rollups by property, technician (split by hours), category, month, plus the
  bill rate that would hit the target margin.

**Taxes** ☑
- Year / quarter tax package: mileage log, expense ledger by category, missing
  receipts, pay by person (W-2 vs 1099-NEC with hours / OT / gross / mileage
  paid back), key dates (federal + California estimates, W-2 / 1099-NEC, return
  due dates), plus the existing 1099 export. CSV + PDF.

**CRM** ☑
- Owners are the property manager's clients: a timeline of notes / calls /
  emails / meetings / issues with pins and **follow-ups** (due follow-ups on the
  dashboard), and an **owner-lead pipeline** (new → contacted → proposal → won /
  lost, by source) for winning new management contracts.

**Print to PDF** ☑
- A real document PDF writer (Helvetica, table layout with column widths,
  right-aligned money, totals, page numbers, the workspace's name on every
  page) replacing the monospace dump — every report, the payroll week, a work
  order's cost sheet and the owner bill.

**Settings** ☑ — overtime rule, labor burden %, overhead per hour, target
margin, mileage rate, maintenance markup, missed-punch hours, clock location +
radius.

## Phase 2C — Properties, maintenance and the parts loop ☐

The next slice: a property is set up in seconds and always has a picture; a
work order carries what the technician found and what it needs; parts flow from
the shopping list to the truck to the unit; and Alpha and Vantedge talk to each
other as vendor and client.

```
 type an address ─► autofill (Photon / Google Places) ─► geocode ─► Street View photo
                                                                     └► nightly job: any property without a photo gets one
 work order ─► findings (+ photos) ─► parts on the finding ─► "generate a parts list"
        │                                                        └► shopping list ─► close-out (night before):
        │                                                             order / pick up / from stock, ship to property or office
        ├─► appliance (asset) ─► its parts catalog + warranty + manuals + service history
        └─► vendor = an Alpha account ─► work request ─► status / photos / bill come back
 inventory: scan-in (camera or scanner gun), receive / use / count, weighted average cost, reorder list
```

**Property autofill & photo** ☐
- Address suggestions as you type: known properties first, then Photon (free,
  no key) or Google Places when a key is in the vault (`google.maps_api_key`);
  picking one fills street, city, state, ZIP; saved properties geocode through
  the existing Census enrichment.
- **Photo**: Street View Static (or the satellite Static Map when no street
  view exists) fetched once, stored as a property document and set as the
  hero; a `property_photo` job runs nightly per workspace and fills in any
  property without a photo (retrying failures after 7 days). Without a key,
  a placeholder is drawn and the job records why.

**Appliances & parts** ☐
- Assets already exist (HVAC, appliances…): add purchase date / price,
  expected life, **warranty** (expiry + provider + document), manuals, and a
  **parts catalog** per asset (inventory items that fit it, with quantity).
- "Replace" / "repair" on an asset starts a work order pre-loaded with its
  parts as *potential parts*.

**Findings, parts lists, shopping list, close-out** ☐
- Work-order **findings** (note kind `finding`, with photos): "baseboards
  rotted behind the washer", each with the parts it needs.
- **Potential parts** on a ticket (from its asset's catalog, or typed);
  **Generate a parts list** merges the ticket's potential parts, findings'
  parts and any in stock → a printable pick list for the truck / Home Depot.
- **Shopping list**: what isn't in stock, per ticket, with a ship-to
  (property / office / other) and a need-by date.
- **Close-out** (the office, the night before): every ticket scheduled
  tomorrow with its shopping list; mark each item *order* (vendor + tracking),
  *pick up*, *from stock* (consumes inventory) or *skip*; ordered items become
  an expense (billable to the owner) and arrive as *received*.

**Inventory** ☐
- Barcode / UPC / SKU lookup (`GET /inventory/lookup?code=`); **scan-in** with
  the camera (BarcodeDetector, zxing fallback) or a scanner gun; receive /
  use / count movements with a landed, **weighted-average unit cost** spread
  across a receipt (tax + shipping); a Friday reorder list by vendor; use on a
  work order from the tech's phone.

**Routine maintenance & the listing** ☐
- Maintenance plans attach to an **asset** (filter change every 90 days, HVAC
  service every spring, chimney sweep every fall) and open the work order with
  the parts pre-listed; each asset shows its service history and spend.
- Property → *Maintenance history*: spend by category and month, per
  appliance, the routine work done and due.
- The public listing surfaces **appliances and upkeep** ("central air, 2023
  water heater, filters changed quarterly") from the asset register.

**Alpha ↔ Vantedge** ☐
- A vendor (counterparty) can be linked to an **Alpha account** (its base URL
  + API key). Dispatching a work order to that vendor sends a **work request**
  to Alpha (`POST /integrations/jobs`, new on the Alpha side); Alpha creates
  the job and posts **status, photos and the bill** back through Vantedge's
  signed inbound webhook (`POST /webhooks/alpha`).
- Vendor API on Vantedge: `GET /api/v1/tickets/{id}`, `PATCH` status /
  comments, and `maintenance_ticket.updated` / `.assigned` / `.resolved`
  webhook events — any vendor system can use them, not only Alpha.

## Phase 3 — Reminders that run themselves ☐

Everything Alpha's scheduler does, translated to rentals. All on the existing
job queue, retimed from Settings → Schedule with run history.

- Rent due (3 days before), rent late (day after grace), autopay failed.
- Lease expiring (90/60/30 days) → kicks off the renewal workflow.
- Maintenance: "tech on the way", "work done — how did we do?" (1–5 reply by
  text, parsed like Alpha's feedback).
- Inspection and move-in/move-out appointment reminders with ICS invites.
- Insurance (renter's policy, vendor COI) expiring.
- Morning summary email for managers: what's due, late, open, flagged.

## Phase 4 — Owner & vendor portals ☐

- **Owner portal** (`/owner`): statements, payouts, property P&L, open work
  orders, documents, approve spend over a limit.
- **Vendor portal** (`/vendor`): the work orders assigned to them, accept /
  schedule / complete with photos, submit a bill (feeds AP), upload W-9 and COI
  with expiry reminders. **Alpha is vendor #1** — and a Vantedge work order can
  be pushed into Alpha as a job (partner integration, both directions).

## Phase 5 — Reviews, renewals marketing & Spanish ☐

- Google reviews on each tenant's public listings site; review asks after a
  completed work order or a lease signing (never gated).
- Listing campaigns: text/email past applicants about a new vacancy (consent).
- Referral credit for residents who refer a renter.
- **Spanish**: resident portal, emails, texts, lease-document templates.
  The High Desert rental market needs it.

## Phase 6 — Field & property tools borrowed from Alpha ☐

- Weather watch (NWS) for scheduled exterior work and freeze alerts to
  residents ("drip your faucets tonight").
- Installable offline PWA for inspections (photos queue while offline).
- Satellite measuring for turn estimates (roof, paint, flooring square feet).
- Equipment/appliance register per unit with service schedules and warranties.

## Phase 7 — Go-live gates (from the partnership letter) ☐

These are gates, not features. Nothing moves forward until the last one is done.

1. LLC formed; operating + license agreement signed.
2. Bree's broker license covers holding others' rent/deposits.
3. CPA reviews the trust accounting.
4. Stripe and Checkr switched from sandbox to live (`LIVE_PROVIDERS`).
5. Hardening: security review, pagination caps, backups + restore drill,
   code escrow and runbook ("this can't all depend on Charles").
6. 90 days running Bree's own portfolio.
7. First outside customer.

## Ideas that fit the family plan

- **Foundation mode**: an affordable-housing workspace with income-limit
  certifications, subsidy/voucher (HAP) payments split from the tenant share,
  and the documented at-cost management fee from the letter.
- **Related-party ledger tag**: every transaction between family entities
  (Bree's company, Alpha, Daedalus IT, the Foundation) is flagged, requires a
  market-rate note and a disinterested approver — rule one of the letter,
  enforced by the software.
- **Land pipeline**: the deals module already covers acquisitions — add a
  "raw land" deal type (acreage, zoning, water/power access, per-acre price)
  for the Lucerne Valley parcels.
- **Holding-company per property** is already modelled (LLCs); add a one-click
  "new property in a new LLC" onboarding.

---

**Still to port from Alpha** ☐ — QuickBooks Online export, equipment meters and
service schedules, inventory movements with weighted-average cost, compliance
documents (insurance, licenses, vendor COIs / W-9s) with expiry reminders, and
offline punches from an installable app.

## Order of work

1. Phase 0 rebrand (copy) — cheap, visible.
2. Phase 1 invite + reset — unblocks every real user.
3. Phase 2 two-way texts core.
4. Everything else in phase order, re-prioritised with Bree after the first
   30 days on her portfolio.
