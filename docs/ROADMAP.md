# Acre Nexus — Roadmap

How we get from today's foundation to the v1 GA described in
[`PRODUCT.md`](./PRODUCT.md). Phases are ordered by **dependency and value**, not
calendar dates; they can be re-prioritised. Each phase lists its goal, the work,
what it unblocks, and a **Definition of Done (DoD)**.

Legend: ✅ shipped · 🟡 partial · ⬜ not started.

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

## Phase 1 — Shared integration substrate ⬜  *(enables 3, 4, 5, 6, real-1, real-2)*

Build the cross-cutting plumbing once so every external integration is uniform.
Full design, grounded in the existing enrichment-engine/audit/PII patterns this
extends: [`docs/INTEGRATIONS.md`](./INTEGRATIONS.md).

- **Secrets/KMS**: per-tenant + platform credential storage (encrypted), surfaced
  to provider clients. Extend the existing PII-key pattern.
- **Object storage + `document` service**: S3-compatible store; a `document`
  entity (polymorphic owner: property / lease / application / entity / deal),
  MIME, size, version, checksum, signed-URL access, retention/expiry, and audit.
- **Outbound provider framework**: a typed `Provider` trait + a **webhook
  ingestion** endpoint + signature verification, all riding the retrying queue
  (the enrichment engine is the reference pattern).
- **Notifications**: transactional email + SMS provider behind the `auto_email`
  job kind; templated, audited.

**DoD:** upload/download a versioned document attached to any entity; a sandbox
webhook round-trips through the queue; a templated email/SMS sends in a test.

---

## Phase 2 — Documents & e-signature (contract signing) ⬜  *(Pillars 3 & 4)*

- **Template → document generation**: render leases/agreements from the theming
  layer's `legal_templates` (merge fields: landlord, tenant, property, terms).
- **E-signature**: native envelope flow (or DocuSign/Dropbox Sign connector) with
  signer roles, status tracking (sent → viewed → signed → completed), and the
  signed PDF stored in the document service. ESIGN/UETA audit trail.
- **Document tracking UI**: per-property/lease/deal document drawer with status,
  versions, and expirations.

**DoD:** generate a lease from a template, send for signature, capture the
completed signed PDF + audit trail, and see it on the property/lease.

---

## Phase 3 — Payments + financial dashboards ⬜  *(Pillar 5)*

- **Processor integration**: Stripe (cards/ACH) + Plaid (bank linking) for rent,
  deposits, and application fees; **autopay** + saved methods (tokenized — no PANs
  stored, PCI-safe).
- **Ledger & reconciliation**: extend `lease_payment` into a proper charges +
  payments ledger; auto-match deposits; late-fee rules; owner **payouts** and
  basic **trust accounting**.
- **Invoices/receipts** + webhook-driven status updates.
- **Charts/dashboards**: time-series rent collected, occupancy, delinquency,
  NOI/cash-flow, portfolio value over time (the KPIs exist; add history + viz).

**DoD:** a tenant pays rent via ACH in sandbox, the ledger + lease status update
from the webhook, an owner payout is computed, and the dashboard charts it.

---

## Phase 4 — Automated background checks (real) ⬜  *(Pillar 1)*

- **FCRA-compliant screening provider** (Checkr / TransUnion SmartMove / similar)
  behind the provider framework: credit + criminal + eviction.
- **Applicant consent** capture, a `screening_report` entity, secure result
  storage, and **adverse-action** workflow + decision recording.
- Wire into the existing apply funnel (replacing the simulated `background_check`
  job) with status surfaced to the back office.

**DoD:** submit an application → real (sandbox) screening runs → report stored →
approve/deny with adverse-action notice, fully audited.

---

## Phase 5 — Tenant lifecycle & resident portal ⬜  *(Pillar 3)*

- **Applicant → tenant conversion**: approved application becomes a lease +
  resident with one action; deposit + first month via Phase 3.
- **Resident portal**: a renter logs in to pay rent, view lease + documents,
  submit maintenance requests, and message the manager (reuses RBAC `renter`).
- **Move-in/move-out**: checklists, inspections (photos via documents), deposit
  disposition.

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
   └─ Phase 1 (substrate)
        ├─ Phase 2 (documents + e-sign) ──┐
        ├─ Phase 3 (payments + charts) ───┼─ Phase 5 (tenant lifecycle/portal)
        ├─ Phase 4 (screening) ───────────┘        └─ Phase 6 (helpdesk)
        └─ Phase 7 (real data)
                         all ─→ Phase 8 (reporting/billing/GA)
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
