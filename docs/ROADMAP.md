# Acre Nexus — Roadmap

How we get from today's foundation to the v1 GA described in
[`PRODUCT.md`](./PRODUCT.md). Phases are ordered by **dependency and value**, not
calendar dates; they can be re-prioritised. Each phase lists its goal, the work,
what it unblocks, and a **Definition of Done (DoD)**.

Legend: ✅ shipped · 🟡 partial · ⬜ not started.

## TODO — what's next

The next slice of work, in dependency order:

- [ ] **Portal round-out (Phase 5)**: lease + documents view and maintenance
      requests in the renter portal (rent payment shipped with Phase 3).
- [ ] **Accounts payable (#58)**: vendor bills → approval → pay, riding the
      Phase 3 ledger + payment execution.
- [ ] **Standard PM reports (#56)**: rent roll, T-12, aging & delinquency on
      top of the new general ledger.
- [ ] **Scale guards**: pagination caps on `GET /applications`,
      `GET /public/listings`, and `GET /my/applications` (the document,
      audit, payment, and ledger lists already cap).
- [ ] **Automated e-sign reminder cadence** (settings-driven schedule + max
      rounds) on top of today's manual remind — the `billing_cycle` job is
      the recurring-scan pattern to follow.

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

## Phase 5 — Tenant lifecycle & resident portal 🟡  *(Pillar 3)*

- **Applicant → tenant conversion** ✅ (shipped with Phase 2): approved
  application becomes a lease with one action — identity/attributes/vehicles
  copied, fees auto-applied, lease document auto-generated, listing closed;
  deposit + first month now collect through the Phase 3 payment pipeline.
- **Resident portal** 🟡: renters already apply white-glove from their
  profile (`/account/profile`), track applications (`/account/applications`),
  maintain vehicles, sign remotely, and **pay rent** — balance, one-click
  pay, saved methods, autopay, receipts (`/account/payments`, shipped with
  Phase 3); still to come — view lease + documents, submit maintenance
  requests, and message the manager.
- **Move-in/move-out** ⬜: checklists, inspections (photos via documents),
  deposit disposition.

**DoD:** an applicant self-serves from approval → signed lease → autopay →
portal, end to end.

---

## Phase 6 — Helpdesk & maintenance ops ⬜  *(Pillar 6)*

- **Support desk**: SLAs, priorities, queues, and resident-facing ticketing on
  top of the maintenance module; threaded comms.
- **External connector** (optional): Zendesk/Intercom sync for tenants who run
  their own helpdesk.
- **Contractor dispatch**: assignment notifications, scheduling, quotes →
  approval → invoice → payment (ties to Phases 1–3).

**DoD:** a resident opens a ticket from the portal, it routes to a contractor
with an SLA, and resolution + cost flow back to the property ledger.

---

## Phase 7 — Real data providers & marketplace depth ⬜  *(Pillar 2, "full")*

- Swap simulated enrichment for **real** county-record / AVM / schools APIs
  behind the existing provider interface (one function each).
- **Media**: property photos/floorplans in the document store.
- Optional: MLS/comps feed, permits/violations, insurance quotes.

**DoD:** a real address enriches from live sources with graceful fallback to
simulation when a provider is unavailable.

---

## Phase 8 — Reporting, billing & GA hardening ⬜

- **Reporting**: owner statements, rent rolls, **1099**/tax exports, portfolio
  analytics.
- **SaaS billing**: meter + bill client workspaces (plans already modeled).
- **Hardening**: security review, **PCI/FCRA/SOC 2** posture, performance, rate
  limiting, observability/metrics, load testing, backup/restore drills.

**DoD:** a paying tenant runs the full lifecycle in production with compliant
controls and monitored SLOs.

---

## Dependency graph (text)

```
Phase 0 ✅
   └─ Phase 1 ✅ (substrate)
        ├─ Phase 2 ✅ (documents + e-sign) ─┐
        ├─ Phase 3 ✅ (payments + charts) ──┼─ Phase 5 🟡 (tenant lifecycle/portal)
        ├─ Phase 4 ✅ (screening) ──────────┘        └─ Phase 6 ⬜ (helpdesk)
        └─ Phase 7 ⬜ (real data)
                         all ─→ Phase 8 ⬜ (reporting/billing/GA)
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
