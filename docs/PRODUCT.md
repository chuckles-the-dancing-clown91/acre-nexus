# Acre Nexus — Product Breakdown

> **One-liner:** a multi-tenant, white-label operating system for property
> investors and managers — "Zillow's data + a property-management back office +
> an investment workflow engine" — sold as SaaS, with services resellable via a
> token API.

This document is the north-star: **what we're building, the end goal, and how
the pieces fit.** The exhaustive capability list (for *total* property
management, beyond the headline pillars) is in [`FEATURES.md`](./FEATURES.md);
the phased plan to get the rest of the way is in [`ROADMAP.md`](./ROADMAP.md).
Deep-dives live in the area docs:
[`ARCHITECTURE`](../ARCHITECTURE.md) · [`IAM`](./IAM.md) · [`AUDIT`](./AUDIT.md) ·
[`PROPERTY_DATA`](./PROPERTY_DATA.md) · [`INVESTING`](./INVESTING.md) ·
[`RENTALS`](./RENTALS.md) · [`MODULES`](./MODULES.md) · [`API`](./API.md).

---

## 1. The end goal

A single platform a real-estate operator logs into to run the **entire lifecycle
of a property** — from sourcing and acquisition, through financing, ownership and
title, to leasing, rent collection, maintenance, and eventual sale or refinance —
with the data, automation, money movement, and documents all in one place.

Three things make it defensible:

1. **Data depth** — every property is automatically enriched with parcel/county
   records, taxes, valuations, schools, utilities, and geocoding, so onboarding a
   house is one form, not a research project.
2. **Workflow + automation** — strategy-aware workflows (rental, flip, BRRRR,
   hold, wholesale) plus a durable background-job engine that runs the slow,
   external, and scheduled work (enrichment, screening, emails, payments).
3. **Composability** — a pluggable module system and a scoped token API, so
   capabilities can be turned on per tenant and individual services can be resold.

**Definition of "done" for v1 GA:** an operator can onboard a property with full
data, take a renter from application → screened → signed lease → autopay, collect
rent and see it on a dashboard, run maintenance with a helpdesk, track every
document, and do it all under audit with role-based access — for multiple client
workspaces under one white-label deployment.

---

## 2. Who it's for (personas)

| Persona | What they do | Status |
|---------|--------------|--------|
| **Acre HQ staff** (platform) | Operate the SaaS: client accounts, support, billing, audit | ✅ |
| **Workspace owner / PM** | Run a portfolio: onboard, finance, lease, maintain | ✅ |
| **Back-office / leasing agent** | Applications, leases, payments, listings | ✅ / 🟡 |
| **Maintenance / contractor** | Work orders assigned to them | ✅ (internal) |
| **Landlord / owner** | Read-only view of their properties, financials, title | ✅ |
| **Renter / applicant** | Apply, sign, pay, request help (resident portal) | ⬜ Planned |
| **Vendor / integrator** | Consume scoped `/api/v1` services | ✅ |

---

## 3. Capability map (status legend: ✅ shipped · 🟡 partial · ⬜ planned)

### Platform foundation — ✅
Multi-tenant (shared schema + `tenant_id` + Postgres RLS), JWT auth + refresh,
fine-grained **RBAC** with runtime-editable roles, encrypted PII, workspace
switching, a **pluggable module system** (per-tenant on/off), an **audit log**
(every request + every state change), a durable **retrying job queue**
(Tokio/Postgres), auto-generated **OpenAPI** docs, and a white-label theming
layer. See `ARCHITECTURE.md`.

### Property data & onboarding — ✅ (real-provider swap‑in: 🟡)
One-call **onboarding** (`POST /properties/onboard`) creates the property +
financing + lender entities + starts the workflow + kicks off enrichment. The
**enrichment engine** fills parcel/county records (APN, zoning, owner, last
sale), tax history, AVM valuation + rent estimate, schools, utilities, and
**live geocoding** (US Census). Providers sit behind one interface — today one is
live and the rest are deterministic simulations; making them all "real" is a
provider swap (see Roadmap). See `PROPERTY_DATA.md`, `INVESTING.md`.

### Investment workflows & financing — ✅
Strategy templates (rental/flip/BRRRR/hold/wholesale) with per-property stage +
history; mortgages/loans that drive **levered cash flow + equity**; an
**entities registry** (banks, lenders, contractors, …) with notes.

### Rentals, maintenance & title — ✅
Units, **leases** (rental + payment status) and a rent **ledger**; **maintenance
work orders** assignable to staff or contractors with a status timeline;
**ownership/deed** records and **liens/encumbrances**. See `RENTALS.md`.

### The six requested pillars — current state → "full"

| # | Pillar | Today | What "full" adds |
|---|--------|-------|------------------|
| 1 | **Automated background checks** | 🟡 Simulated screening job (durable state machine: submit → await → cleared) on the apply funnel | Real **FCRA-compliant** provider (e.g. Checkr / TransUnion SmartMove): credit + criminal + eviction, applicant consent capture, a `screening_report` record, and **adverse-action** handling + decisioning |
| 2 | **Property onboarding w/ full data** | ✅ Onboarding + enrichment (parcel #, schools, taxes, valuation, utilities, geo) | Swap simulated sources for real county/AVM/schools APIs; add **photos/media** + onboarding **document** capture; richer hazard/flood/permits |
| 3 | **Tenant management + onboarding + contract signing** | 🟡 Leases + tenant identity exist | Applicant → tenant **conversion**, lease **document generation** from templates (the theming layer already stores legal templates), **e-signature** (native or DocuSign), and a **tenant/resident portal** |
| 4 | **Rental + purchase, with document tracking** | 🟡 Rentals shipped; purchase via workflow + financing | A first-class **transaction/deal** object for purchases/sales, and a **document management system** (object storage, per-entity attachments, versions, e-sign status, expirations) |
| 5 | **Payment integration + charts** | 🟡 Rent ledger + portfolio KPIs | **Stripe/ACH (+ Plaid)** for rent + deposits + autopay, invoices/receipts, payouts to owners, reconciliation, and **time-series dashboards/charts** |
| 6 | **Helpdesk integration** | 🟡 Internal maintenance tickets + timeline | A **resident support desk** (ticketing with SLAs + comms) and/or a connector to **Zendesk/Intercom**, plus contractor dispatch/scheduling |

### Leasing & public site — ✅ / 🟡
White-label listings site + application funnel + (simulated) screening + automated
emails. "Full" = the screening + tenant-onboarding upgrades above.

### Vendor API — ✅
Scoped, revocable tokens powering `/api/v1` so services are resellable.

---

## 4. Cross-cutting foundations the "full" pillars depend on

These are shared capabilities several pillars need; building them once unblocks
many features (sequenced in the Roadmap):

- **Secrets & KMS** — real integrations (screening, payments, e-sign, data
  providers) need managed API keys; PII key handling already exists as the model.
- **Object storage & document service** — S3-compatible store + a `document`
  entity (owner = property/lease/application/entity), versions, MIME, e-sign
  status. Unblocks pillars 2, 3, 4.
- **Outbound integrations framework** — typed provider trait + webhook
  ingestion + the existing retrying queue. Unblocks 1, 5, 6.
- **Notifications** — transactional email + SMS (the auto-email job is the seed).
- **Money primitives** — already integer-cents throughout; add processor +
  reconciliation + double-entry-ish ledger for trust accounting.
- **Compliance** — FCRA (screening/adverse action), ESIGN/UETA (e-sign), PCI
  (payments → tokenize, never store PANs), SOC 2 posture (audit log is in place).

---

## 5. Architecture in one paragraph

Rust **Rocket** API over **SeaORM/Postgres**, assembled from pluggable
`PlatformModule`s; a **Tokio** durable job queue runs background work; a Next.js
App-Router frontend mirrors the module registry for nav + gating. Every request
is audited; every tenant-scoped row carries `tenant_id` with RLS as defence in
depth; OpenAPI is generated from the code. New capabilities land as **a module +
a migration + entities + per-handler route files + a frontend page** — the
pattern every feature in §3 already follows. Full detail in `ARCHITECTURE.md`.
