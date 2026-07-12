# Acre Nexus — Roadmap

How we get from today's foundation to the v1 GA described in
[`PRODUCT.md`](./PRODUCT.md). Phases are ordered by **dependency and value**, not
calendar dates; they can be re-prioritised. Each phase lists its goal, the work,
what it unblocks, and a **Definition of Done (DoD)**.

Legend: ✅ shipped · 🟡 partial · ⬜ not started.

## TODO — what's next

The next slice of work, in dependency order:

- [x] **Prod-safety config guards (T0 · #23/#24/#25)**: production
      (`APP_ENV=production`) now **fails closed** — `JWT_SECRET`, `PII_ENC_KEY`,
      and `SECRETS_ENC_KEY` must be explicitly set (no derive-from-`JWT_SECRET`
      fallback, weak/default `JWT_SECRET` rejected), `AUTO_MIGRATE` defaults off,
      and demo data is never seeded in prod. See
      [`backend/README.md`](../backend/README.md#production-safety-app_envproduction).
- [x] **Portal round-out (Phase 5)**: lease + documents view, maintenance
      requests, messaging, move-in/move-out inspections, and security-deposit
      disposition — shipped, see [`PORTAL.md`](PORTAL.md).
- [x] **Accounts payable (#58)**: vendor bills → approval → pay, riding the
      Phase 3 ledger + payment execution — shipped, see
      [`PAYMENTS.md`](PAYMENTS.md#accounts-payable-vendor-bills).
- [x] **Platform services phase (#54/#62/#68)**: the calendar/reminders
      engine ([`CALENDAR.md`](CALENDAR.md)), inbound email→ticket/lead +
      SPF/DKIM/DMARC deliverability ([`EMAIL.md`](EMAIL.md)), and vendor
      outbound webhooks ([`WEBHOOKS.md`](WEBHOOKS.md)).
- [x] **Leasing CRM & lease renewals (#44)**: closes the last gap in
      [`FEATURES.md`](FEATURES.md) §2 — the pre-lease **CRM** prospect pipeline
      (leads → tours → one-click convert-to-application) and the ongoing
      **lease-renewal** workflow (propose → addendum → e-sign → auto-apply the
      new rent + term), riding the Phase 2 document/e-sign substrate. See
      [`LEASING.md`](LEASING.md#lease-renewals-issue-44). *(Remaining §2: listing
      syndication to Zillow/Apartments.com/MLS, self-showing/lockboxes.)*
- [ ] **Standard PM reports (#56)**: rent roll, T-12, aging & delinquency on
      top of the new general ledger.
- [ ] **Scale guards**: pagination caps on `GET /applications`,
      `GET /public/listings`, and `GET /my/applications` (the document,
      audit, payment, and ledger lists already cap).
- [ ] **Automated e-sign reminder cadence** (settings-driven schedule + max
      rounds) on top of today's manual remind — the reminders engine (#54)
      is now the natural home.

---

## Phase 0 — Foundation ✅ (shipped on this branch / PR #3)

Multi-tenant platform, IAM + RBAC, encrypted PII, audit logging, pluggable
modules, durable retrying job queue, OpenAPI, white-label theming; property
management + portfolio; **property intelligence/enrichment** (parcel, tax,
valuation, schools, utilities, live geocoder); **investor onboarding**, entities
registry, financing (mortgages → cash flow + equity), investment workflows;
**rentals** (units/leases/ledger), **maintenance** (work orders), **title**
(ownership + liens); vendor token API.

**DoD:** ✅ all of the above compiling, clippy-clean, tested, documented, seeded.

---

## Phase 1 — Shared integration substrate ✅  *(enables 3, 4, 5, 6, real-1, real-2)*

Build the cross-cutting plumbing once so every external integration is uniform.
**Shipped** — see [`INTEGRATIONS.md`](INTEGRATIONS.md) and
[`NOTIFICATIONS.md`](NOTIFICATIONS.md) for the as-built design.

- **Secrets/KMS**: per-tenant + platform credential storage (encrypted), surfaced
  to provider clients. Extends the PII-key pattern under a dedicated,
  fail-closed `SECRETS_ENC_KEY`.
- **Object storage + `document` service**: S3-compatible store (local dev/CI
  backend + SigV4 presigning); a `document` entity (polymorphic owner: property /
  lease / application / entity / deal), MIME, size, version, checksum,
  signed-URL access, retention/expiry, and audit.
- **Outbound provider framework**: a typed `Provider` trait + a **webhook
  ingestion** endpoint + signature verification, all riding the retrying queue
  (the enrichment engine is the reference pattern).
- **Notifications** (shipped beyond original scope): templated email + SMS +
  **Web Push** (VAPID/RFC 8291) + **chat** (Slack/Discord) + a per-user
  **in-app inbox**, with tenant-configurable delivery providers (Resend,
  SendGrid, Postmark, Twilio) behind the `auto_email`/`auto_sms`/`auto_push`/
  `auto_chat` job kinds; templated, idempotent, audited.

**DoD (met):** upload/download a versioned document attached to any entity; a
sandbox webhook round-trips through the queue; a templated email/SMS sends in a
test.

---

## Phase 2 — Documents & e-signature (contract signing) ✅  *(Pillars 3 & 4)*

**Shipped** — see [`LEASING.md`](LEASING.md#e-signature-envelopes) for the
as-built design.

- **Template → document generation** ✅: leases render from the theming layer's
  `legal_templates` (merge fields: landlord, tenant, property, terms, charges,
  pets, vehicles).
- **E-signature** ✅: native envelope flow (`esign_envelope` / `esign_signer` /
  `esign_event`) with signer roles (resident / landlord / guarantor / other),
  status tracking (sent → viewed → signed → completed, plus declined/voided),
  tokenized public signing links delivered by **email + SMS** through the
  Phase 1 notification substrate, a hand-rolled text→PDF writer, and the signed
  PDF stored in the document service. Full ESIGN/UETA audit trail (typed
  signature + consent + IP + user agent + SHA-256 body hash pinned at send).
  Completion auto-activates the lease, syncs occupancy, and advances the
  property's workflow to `leased`.
- **Document tracking UI** ✅: per-property and per-lease document drawer with
  status, version chains, and expirations; envelope card with per-signer
  status, reminders, void, and the audit trail; public `/sign/<token>` signing
  page. Notification templates are importable into the workspace and editable
  from the console.
- **Post-ship hardening** ✅: concurrent final signatures serialize on a row
  lock; in-person signing voids open envelopes (and vice-versa a signed
  document refuses a stale link); profile updates merge instead of
  full-replace; "viewed" is interaction-driven (scanners don't pollute the
  trail); a storage outage degrades to a retryable `esign_store_pdf` job;
  declined envelopes reopen the listing. Every pipeline mutation now writes a
  domain audit event (`application.screened`, `listing.sync`,
  `lease.activate`, occupancy sync, auto workflow advance, auto void, signed
  PDF store), and the flow's tunables are per-tenant **settings** — screening
  policy (credit floor, income multiple, callback pace), signing-link expiry,
  signer cap, signed-PDF retention, document title, auto-generate-on-convert
  (see [LEASING.md](LEASING.md#workspace-settings)).

**DoD (met):** generate a lease from a template, send for signature, capture
the completed signed PDF + audit trail, and see it on the property/lease.

---

## Phase 3 — Payments + accounting core + financial dashboards ✅  *(Pillar 5)*

**Shipped** — see [`PAYMENTS.md`](PAYMENTS.md) for the as-built design.

- **Accounting core** ✅: double-entry general ledger + chart of accounts per
  legal entity (`ledger_account`/`ledger_txn`/`ledger_entry`), a single
  validated posting path (balance enforced, so a trial balance always sums
  to zero), **trust accounting** with the no-commingling invariant enforced
  in the posting engine, and trial balance / income statement / trust
  reconciliation reports.
- **Processor integration** ✅: Stripe (cards/ACH) behind the Phase 1
  provider framework — sandbox-first, live via `LIVE_PROVIDERS` — with
  tokenized saved methods (no PANs stored), **autopay**, and webhook-driven
  settlement; Plaid bank linking + transaction feeds with auto-matching
  reconciliation and a manual match/ignore review queue.
- **Ledger & reconciliation** ✅: `lease_payment` grew into a receivable +
  payment lifecycle (`due → processing → paid/failed`, plus `late`); a
  recurring per-tenant **billing cycle** raises rent receivables, assesses
  settings-driven **late fees** (grace, flat + percent, one-time/daily,
  capped), runs autopay, and refreshes bank feeds.
- **Payouts, receipts & statements** ✅: owner draws computed from the
  entity's actual books (rent collected − expenses − management fee),
  executed as ACH via the provider, posted to the ledger, with a generated
  owner-statement PDF; every settled payment issues a receipt PDF into the
  document service.
- **Charts/dashboards** ✅: `GET /finance/series` merges live ledger rollups
  (rent due/collected, NOI) with monthly snapshot history (occupancy,
  delinquency, portfolio value); the console dashboard renders 12-month
  trends with a dependency-free SVG chart, and the renter portal gained a
  full **pay-rent** page (balance, one-click pay, methods, autopay,
  receipts).

**DoD (met):** a resident pays rent from the portal (or autopay collects it),
the payment settles through the provider pipeline and updates the lease +
posts a balanced ledger entry + issues a receipt; a trial balance sums to
zero and the trust ledger reconciles; an owner payout computed from the
ledger executes in sandbox and files its statement; the dashboard charts a
year of trends from real ledger data.

---

## Phase 4 — Automated background checks (real) ✅  *(Pillar 1)*

**Shipped** — see [`SCREENING.md`](SCREENING.md) for the as-built design.

- **FCRA screening provider** ✅: Checkr behind the Phase 1 provider framework
  (credit + criminal + eviction) — deterministic sandbox by default,
  `LIVE_PROVIDERS=checkr` + the `checkr.api_key` credential for real reports,
  completion by signature-verified webhook (`POST /webhooks/checkr`).
- **Consent + report + adverse action** ✅: FCRA §604(b) consent captured at
  every intake door (no consent → no application), a `screening_report` entity
  per application, and the §615(a) **adverse-action** workflow — notice
  generated + filed as a PDF document, emailed to the applicant, stamped on the
  application, auto-sent on decline (setting) or sent from the console.
- **Wired into the apply funnel** ✅: the `background_check` job now orders a
  real report and lands its policy verdict (credit floor, income multiple,
  criminal/eviction records) through the same `application.screened` slot;
  the report surfaces in the back office behind the new `screening:read`
  permission.

**DoD:** ✅ submit an application → (sandbox) screening runs → report stored →
approve/deny with adverse-action notice, fully audited.

---

## Phase 5 — Tenant lifecycle & resident portal ✅  *(Pillar 3)*

**Shipped** — see [`PORTAL.md`](PORTAL.md) for the as-built design.

- **Applicant → tenant conversion** ✅ (shipped with Phase 2): approved
  application becomes a lease with one action — identity/attributes/vehicles
  copied, fees auto-applied, lease document auto-generated, listing closed;
  deposit + first month now collect through the Phase 3 payment pipeline.
- **Resident portal** ✅: renters apply white-glove from their profile
  (`/account/profile`), track applications (`/account/applications`),
  maintain vehicles, sign remotely, **pay rent** (`/account/payments`,
  Phase 3), and now — **view their lease + documents** (signed lease,
  receipts, statements via audited signed-URL downloads, `/account/lease`),
  **submit maintenance requests** with photos and a live timeline
  (`/account/maintenance`), and **message the manager** (threaded
  conversations answered from the console, `/account/messages`), all through
  identity-scoped `/my/*` routes.
- **Move-in/move-out** ✅: move-in/move-out **inspections** with a standard
  condition checklist + photos via the document service, and
  **security-deposit disposition** — itemized deductions posted through the
  trust ledger, the refund executed on the provider payout rail, and a
  generated statement PDF filed on the lease + emailed to the resident.

**DoD (met):** an applicant self-serves from approval → signed lease →
autopay → portal, end to end — and moves out with an inspected unit and a
settled, statemented deposit.

---

## Phase 6 — Helpdesk & maintenance ops ✅  *(Pillar 6)*

**Shipped** — see [`HELPDESK.md`](HELPDESK.md) for the as-built design.

- **Support desk** ✅: per-priority **SLA policy** (first-response +
  resolution targets stamped on every ticket, re-stamped on priority change,
  breach states on the board and detail view), a per-tenant `helpdesk_scan`
  job surfacing breaches to staff, and resident-facing ticketing + threaded
  comms (shipped with Phase 5).
- **Contractor dispatch** ✅: assignment notifications (member in-app+email,
  contractor dispatch email with schedule + scope), scheduling on the
  ticket, **quotes → approval** (approval gated like vendor bills, feeding
  the ticket's cost) → **invoice → payment** through the Phase 3
  accounts-payable loop, landing on the property ledger.
- **Preventive maintenance + turnover** ✅: recurring `maintenance_plan`s
  auto-open tickets on cadence; completing a move-out inspection auto-opens
  a make-ready ticket and flags the unit (setting-gated).
- **External connector** (optional): deferred — the Phase 1 provider
  framework is the natural home when a client runs an external desk.

**DoD (met):** a resident opens a ticket from the portal, it routes to a
contractor with an SLA, and resolution + cost flow back to the property
ledger.

---

## Phase 7 — Real data providers & investor depth 🟡  *(Pillar 2, "full")*

- [x] **Acquisitions & underwriting (#41/#42)** — shipped: the `flips` module is
      now GA with a real `deal` domain (a buy-side pipeline `prospecting → offer
      → under_contract → closing → owned`), an investor-grade underwriting engine
      (cap rate / cash-on-cash / IRR / DSCR + rent-growth sensitivity), a
      due-diligence checklist + data room, and one-click conversion into an owned
      property. See [`DEALS.md`](DEALS.md).
- [x] **Real data providers + graceful fallback** — shipped: the live Census
      geocoder now returns real coordinates **+ county / FIPS**, and every source
      **gracefully falls back to simulation** when a provider is unavailable
      (recording which provider actually served it). See
      [`PROPERTY_DATA.md`](PROPERTY_DATA.md).
- [x] **Media** — property photos/floorplans in the document store, with a hero
      and a gallery rendered on the property profile.
- [x] **Rehab / construction management (#40)** — shipped: the `rehab` module —
      renovation budgets, scope lines, change orders, draw requests with progress
      photos, and generated lien waivers. See [`REHAB.md`](REHAB.md).
- [ ] Remaining real vendors (AVM / schools / county assessor) behind the
      provider seam; MLS/comps feed, permits/violations, insurance quotes.
- [ ] Disposition / broker (#43), map / geospatial portfolio view (#57).

**DoD (met):** a real address enriches from live sources with graceful fallback
to simulation when a provider is unavailable; photos render on the property
profile. *(Remaining: more real vendors + the #40/#43/#57 sub-issues.)*

---

## Phase 8 — Reporting, billing & GA hardening 🟡

- [x] **Standard PM reports (#56)** — shipped: the `reports` module — rent roll,
      T-12 (off the general ledger), AR aging, and delinquency, each with CSV/PDF
      export. See [`REPORTS.md`](REPORTS.md).
- [x] **Owner statements + 1099/tax exports** — shipped: a cash-basis owner
      statement per legal entity (reconciling with owner payouts) and the annual
      1099-NEC (vendors) + 1099-MISC (owner rents) export, both CSV/PDF. See
      [`REPORTS.md`](REPORTS.md).
- [ ] **Reporting** (rest): portfolio analytics, custom report builder.
- [x] **Global search (#55)** — shipped: the `search` module — a permission-aware
      command palette across properties, tenants, entities, tickets, and LLCs.
- [x] **SaaS billing** — shipped: per-door metered subscriptions (three plans,
      base fee + per-unit overage), automatic monthly `platform_invoice`
      generation, a workspace self-serve subscription/invoice view, and an Acre
      HQ billing console (MRR, plan management, billing run, settle/void). See
      [`SAAS_BILLING.md`](SAAS_BILLING.md).
- [ ] **Hardening**: security review, **PCI/FCRA/SOC 2** posture, performance,
      observability/metrics, load testing, backup/restore drills.
  - [x] **Rate limiting (#67)** — shipped: a fixed-window Rocket fairing with a
        tight auth bucket + generous general bucket, `X-RateLimit-*` headers, and
        `429` + `Retry-After` on breach. See [`RATE_LIMITING.md`](RATE_LIMITING.md).
- [ ] MFA/2FA, SSO/SAML/SCIM (enterprise); GDPR/CCPA data requests.

**DoD:** a paying tenant runs the full lifecycle in production with compliant
controls and monitored SLOs. *(Reports, search, and SaaS billing done; hardening +
the rest remain.)*

---

## Dependency graph (text)

```
Phase 0 ✅
   └─ Phase 1 ✅ (substrate)
        ├─ Phase 2 ✅ (documents + e-sign) ─┐
        ├─ Phase 3 ✅ (payments + charts) ──┼─ Phase 5 ✅ (tenant lifecycle/portal)
        ├─ Phase 4 ✅ (screening) ──────────┘        └─ Phase 6 ✅ (helpdesk)
        └─ Phase 7 🟡 (acquisitions ✅ · real data + media ✅ · rehab ✅ · #43/#57 ⬜)
                         all ─→ Phase 8 🟡 (PM reports + search + SaaS billing ✅ · hardening ⬜)
```

## Sequencing notes

- **Phase 1 is the unlock**: documents, webhooks, secrets, and notifications are
  prerequisites for almost every "full" pillar — do it first.
- Pillars can ship **per-module behind flags** (the module system already
  supports per-tenant enablement + `preview`), so we can release incrementally
  and pilot with one workspace.
- Every external integration must be **sandbox-first and credential-gated**, with
  a simulated fallback (the enrichment engine already proves this pattern) so CI
  stays hermetic and demos work offline.
- Compliance is not a phase you bolt on — FCRA (P4), ESIGN (P2), PCI (P3) are
  built into those phases' DoD.
