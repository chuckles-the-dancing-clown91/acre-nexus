# Acre Nexus — Feature Catalog (Total Property Management)

The exhaustive map of capabilities for a platform that manages a property's
**entire** lifecycle — for landlords, property managers, flippers, and
investors. This is broader than the six headline pillars in
[`PRODUCT.md`](./PRODUCT.md); it's the full surface we'd want for "total
management," so we can prioritise deliberately rather than discover gaps late.

Status: ✅ shipped · 🟡 partial / foundation exists · ⬜ planned.
Priority: **P1** (core to GA) · **P2** (fast-follow) · **P3** (differentiator / later).
Most items map to a phase in [`ROADMAP.md`](./ROADMAP.md).

---

## 1. Accounting & finance
The biggest gap for "total" management — most operators live in their books.

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Rent ledger (charges + payments) | 🟡 | P1 | `lease_payment` exists; grow into charges/credits ledger |
| Online payments (ACH/card) + autopay | ⬜ | P1 | Stripe + Plaid; tokenized, PCI-safe |
| Double-entry **general ledger** | ⬜ | P1 | per-LLC books; the integrity backbone for everything below |
| Chart of accounts (customizable) | ⬜ | P1 | GAAP-ish defaults + per-tenant overrides |
| **Trust / escrow accounting** | ⬜ | P1 | legally required for PMs holding client funds in many states |
| Bank accounts + **bank feeds & reconciliation** | ⬜ | P1 | Plaid; auto-match deposits |
| Accounts payable (vendor bills → approval → pay) | ⬜ | P2 | ties to vendors + work orders |
| Owner **draws/distributions** + contributions | ⬜ | P2 | owner payouts, statements |
| **Owner statements** + owner portal | ⬜ | P2 | monthly packets, ledgers |
| Budgets, forecasts, pro formas | ⬜ | P2 | per property/portfolio |
| Investment underwriting calculators | 🟡 | P2 | cap rate, CoC, IRR, DSCR; cash-flow/equity already computed |
| Late fees / fee schedules / autobill | ⬜ | P2 | rules engine on the ledger |
| Receipt capture + OCR, expense tracking | ⬜ | P3 | document service + AI |
| Depreciation schedules | ⬜ | P3 | tax basis tracking |
| **1099** (vendor + owner), Schedule E, tax exports | ⬜ | P2 | year-end compliance |
| Multi-entity consolidation / inter-company | ⬜ | P3 | LLCs already modeled |

## 2. Leasing, marketing & CRM

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Public white-label listings site | ✅ | — | per-tenant branded |
| Online applications + application fees | 🟡 | P1 | apply funnel exists; add fee payment |
| **Tenant screening** (credit/criminal/eviction/income) | 🟡 | P1 | simulated job today → real FCRA provider |
| Leasing **CRM** (leads, tours, follow-ups) | ⬜ | P2 | prospect pipeline + comms |
| Listing **syndication** (Zillow, Apartments.com, MLS) | ⬜ | P2 | feed out to portals |
| Tour scheduling / self-showing / lockboxes | ⬜ | P3 | calendar + access |
| Rent **pricing / comps** optimization | 🟡 | P3 | AVM rent estimate exists |
| Vacancy / days-on-market tracking | 🟡 | P2 | unit status exists |
| Lease **renewals & rent increases** workflow | ⬜ | P1 | notices + e-sign |
| Waitlists | ⬜ | P3 | |

## 3. Tenant / resident experience

| Feature | Status | Pri | Notes |
|---|---|---|---|
| **Resident portal** (pay, requests, docs, messages) | ⬜ | P1 | renter persona exists in RBAC |
| Applicant → tenant **conversion** | ⬜ | P1 | one action: approved app → lease |
| **Lease generation** from templates + **e-sign** | ✅ | — | envelopes, signer links, ESIGN audit trail, signed PDF |
| Renewals, amendments, addenda | ⬜ | P2 | versioned docs + e-sign |
| **Security deposit** mgmt + disposition + statements | ⬜ | P1 | escrow accounting tie-in |
| Renters **insurance** tracking / requirement | ⬜ | P2 | policy expiry reminders |
| Communications: email / SMS / in-app + broadcast | ⬜ | P1 | `auto_email` job is the seed |
| Move-in / move-out **inspections** w/ photos | ⬜ | P2 | condition reports |
| Package mgmt, amenity booking, community board | ⬜ | P3 | multifamily nice-to-haves |

## 4. Maintenance & operations

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Work orders + assignment + timeline | ✅ | — | staff or contractor |
| **Helpdesk / resident support** (SLAs, queues) | 🟡 | P1 | tickets exist; add SLA + resident-facing |
| Vendor management + **dispatch** + scheduling | 🟡 | P2 | entities registry exists |
| Vendor **bids → approval → invoice → pay** | ⬜ | P2 | ties to AP |
| **Preventive maintenance** schedules | ⬜ | P2 | recurring jobs (queue exists) |
| Inspections (routine, drive-by, turnover) | ⬜ | P2 | templated checklists |
| **Make-ready / turnover** workflow | 🟡 | P2 | unit `make_ready` status exists |
| Asset / appliance / warranty tracking | ⬜ | P3 | serials, warranty expiry |
| Recurring service contracts (landscaping, pest) | ⬜ | P3 | |
| Emergency / on-call routing | ⬜ | P3 | |

## 5. Acquisition, disposition & capital projects (flip/invest)

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Deal pipeline / strategy workflows | ✅ | — | rental/flip/BRRRR/hold/wholesale stages |
| Underwriting models + sensitivity | 🟡 | P2 | calculators expansion |
| Offers / LOIs / purchase contracts | ⬜ | P2 | deal object + e-sign |
| Due-diligence checklists + **data room** | ⬜ | P2 | document service |
| Closing mgmt (title, escrow, settlement) | ⬜ | P3 | title/liens modeled already |
| **Rehab / construction** mgmt (scope, draws, change orders, lien waivers, progress photos) | ⬜ | P2 | core to flips |
| CapEx budgeting + draws | ⬜ | P2 | |
| 1031 exchange tracking | ⬜ | P3 | |
| Disposition / sale workflow + broker mgmt | 🟡 | P3 | flip "listed/sold" stages exist |

## 6. Compliance, legal & risk

| Feature | Status | Pri | Notes |
|---|---|---|---|
| **Document management** (storage, versions, expiry) | ✅ | — | document service + per-record drawer UI |
| Notices (late/cure/entry/eviction) generation + delivery | ⬜ | P2 | templated + audit |
| **Eviction** case tracking | ⬜ | P2 | |
| Licenses / permits / rental registrations + renewals | ⬜ | P2 | expiry reminders |
| Insurance policy tracking (property/liability/flood) | ⬜ | P2 | |
| Fair Housing / rent-control / local ordinance guards | ⬜ | P2 | jurisdiction rules |
| Habitability / code-violation tracking | ⬜ | P3 | |
| Audit trail | ✅ | — | every request + change |
| Data retention + GDPR/CCPA requests | ⬜ | P3 | |

## 7. Portfolio analytics & reporting (charts)

| Feature | Status | Pri | Notes |
|---|---|---|---|
| KPI dashboards | 🟡 | P1 | portfolio summary exists; add **time-series charts** |
| Rent roll, T-12, aging, delinquency reports | ⬜ | P1 | standard PM reports |
| Custom report builder + scheduled exports | ⬜ | P3 | |
| Owner / investor reporting | ⬜ | P2 | statements, K-1s for syndications |
| Performance analytics + benchmarking | ⬜ | P3 | |
| Map / geospatial portfolio view | ⬜ | P3 | lat/long already enriched |
| Data export / API / warehouse sync | 🟡 | P2 | token API exists |

## 8. Property data & intelligence

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Enrichment: parcel, tax, valuation, schools, utilities, geo | ✅ | — | live geocoder + simulated providers |
| Swap simulated → **real** county/AVM/schools APIs | 🟡 | P2 | one-function provider swap |
| Comps / market trends / rent estimates | 🟡 | P3 | AVM exists |
| Hazard/flood/crime/demographics/permits | 🟡 | P3 | flood zone exists |
| **Photos / media / floor plans / virtual tours** | ⬜ | P1 | document/media service |
| Lease **abstraction** (AI extract terms from PDFs) | ⬜ | P3 | document AI |

## 9. Platform, integrations & cross-cutting

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Multi-tenant + RBAC + workspaces | ✅ | — | runtime-editable roles |
| Pluggable modules (per-tenant on/off) | ✅ | — | |
| White-label theming + **custom domains** | 🟡 | P2 | `custom_domain` field exists |
| Durable background job queue | ✅ | — | retrying, scheduled |
| **Notifications** (email/SMS/push) + preferences | 🟡 | P1 | auto-email seed |
| **Document storage + e-sign** | ✅ | — | Phase 1 storage + Phase 2 envelopes |
| **Payments** processor + webhooks | ⬜ | P1 | |
| Outbound integration / webhook framework + **secrets/KMS** | ⬜ | P1 | enables every real provider |
| Public/partner API + webhooks + integration marketplace | 🟡 | P2 | scoped token API exists |
| **Mobile apps** (manager / resident / inspector, offline) | ⬜ | P3 | |
| **MFA/2FA**, SSO/SAML/SCIM (enterprise) | ⬜ | P2 | |
| Global **search** | ⬜ | P2 | |
| **AI copilot** (leasing chat, maintenance triage, comms drafting, report Q&A) | ⬜ | P3 | strong differentiator |
| **Import / migration** (Buildium/AppFolio/Yardi/CSV) | ⬜ | P2 | adoption unlock |
| SaaS **billing/metering** for client workspaces | ⬜ | P2 | plans modeled |
| Calendar / scheduling / reminders engine | ⬜ | P2 | leases, inspections, renewals |

## 10. Optional verticals (expand TAM)

| Feature | Status | Pri | Notes |
|---|---|---|---|
| **Investor / syndication** suite (cap table, capital calls, waterfalls, distributions, K-1s, investor portal) | ⬜ | P3 | if targeting GP/LP |
| **HOA / association** mgmt (dues, violations, ARC requests, board/voting) | ⬜ | P3 | distinct buyer |
| **Short-term rental** (channel mgr, dynamic pricing, cleaning turns) | ⬜ | P3 | Airbnb/VRBO |
| Affordable / LIHTC compliance | ⬜ | P3 | heavy compliance |
| Commercial (CAM reconciliation, percentage rent) | ⬜ | P3 | different lease math |

---

## What I'd build next (highest leverage)

1. **Integration substrate** (secrets/KMS · object storage + `document` service ·
   webhook framework · notifications) — Roadmap Phase 1. Unblocks documents,
   payments, screening, helpdesk, and real data providers at once.
2. **Documents + e-signature** — contract signing + document tracking (two of the
   six pillars) sit directly on the substrate.
3. **Accounting core** (general ledger + trust accounting + bank rec) alongside
   **payments** — this is the difference between a CRM and a true PM platform, and
   most competitors win or lose here.
4. **Resident portal + tenant lifecycle** — pay/sign/request in one place.
5. **Real screening** and **helpdesk/SLAs** — finish the remaining headline pillars.

Everything here lands as the established pattern: **a module + migration +
entities + per-handler routes + a frontend page**, gated per-tenant and audited.
