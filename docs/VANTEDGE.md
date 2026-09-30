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

## Order of work

1. Phase 0 rebrand (copy) — cheap, visible.
2. Phase 1 invite + reset — unblocks every real user.
3. Phase 2 two-way texts core.
4. Everything else in phase order, re-prioritised with Bree after the first
   30 days on her portfolio.
