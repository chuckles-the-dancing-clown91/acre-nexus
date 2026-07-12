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
| Accounts payable (vendor bills → approval → pay) | ✅ | — | `vendor_bill`: ticket → bill → approve (accrues) → pay (clears AP) |
| Owner **draws/distributions** + contributions | ⬜ | P2 | owner payouts, statements |
| **Owner statements** + owner portal | ⬜ | P2 | monthly packets, ledgers |
| Budgets, forecasts, pro formas | ⬜ | P2 | per property/portfolio |
| Investment underwriting calculators | ✅ | — | cap rate, CoC, IRR, DSCR + rent-growth sensitivity, on the acquisition `deal` (`docs/DEALS.md`) |
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
| Leasing **CRM** (leads, tours, follow-ups) | ✅ | P2 | `lead` pipeline (inbound-email + manual entry) + tour scheduling + one-click convert-to-application (#44); see `LEASING.md` |
| Listing **syndication** (Zillow, Apartments.com, MLS) | ⬜ | P2 | feed out to portals — the remaining §2 gap |
| Tour scheduling / self-showing / lockboxes | 🟡 | P3 | tour scheduling ships as `tour` calendar reminders off a lead (#44); self-showing / lockbox access still open |
| Rent **pricing / comps** optimization | 🟡 | P3 | AVM rent estimate exists |
| Vacancy / days-on-market tracking | 🟡 | P2 | unit status exists |
| Lease **renewals & rent increases** workflow | ✅ | P1 | propose → addendum → e-sign → auto-apply the new rent + term, riding the Phase 2 e-sign substrate (#44); see `LEASING.md` |
| Waitlists | ⬜ | P3 | |

## 3. Tenant / resident experience

| Feature | Status | Pri | Notes |
|---|---|---|---|
| **Resident portal** (pay, requests, docs, messages) | ✅ | P1 | Phase 5 — see `PORTAL.md` |
| Applicant → tenant **conversion** | ⬜ | P1 | one action: approved app → lease |
| **Lease generation** from templates + **e-sign** | ✅ | — | envelopes, signer links, ESIGN audit trail, signed PDF |
| Renewals, amendments, addenda | ⬜ | P2 | versioned docs + e-sign |
| **Security deposit** mgmt + disposition + statements | ✅ | P1 | Phase 5 — trust-ledger postings + refund + statement PDF |
| Renters **insurance** tracking / requirement | ⬜ | P2 | policy expiry reminders |
| Communications: email / SMS / in-app + broadcast | 🟡 | P1 | outbound + inbound email→ticket/lead + comms log shipped (`docs/EMAIL.md`) |
| Move-in / move-out **inspections** w/ photos | ✅ | P2 | Phase 5 — checklist + document-service photos |
| Package mgmt, amenity booking, community board | ⬜ | P3 | multifamily nice-to-haves |

## 4. Maintenance & operations

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Work orders + assignment + timeline | ✅ | — | staff or contractor |
| **Helpdesk / resident support** (SLAs, queues) | ✅ | P1 | Phase 6 — SLA policy + breach scan, resident ticketing (Phase 5); see `HELPDESK.md` |
| Vendor management + **dispatch** + scheduling | ✅ | P2 | Phase 6 — dispatch notifications, quotes → approval → bill |
| Vendor **bids → approval → invoice → pay** | 🟡 | P2 | invoice → approval → pay shipped (AP); bids still open |
| **Preventive maintenance** schedules | ✅ | P2 | Phase 6 — `maintenance_plan` + helpdesk scan |
| Inspections (routine, drive-by, turnover) | 🟡 | P2 | move-in/move-out shipped (Phase 5); routine/drive-by open |
| **Make-ready / turnover** workflow | ✅ | P2 | Phase 6 — auto turnover ticket + unit flag on move-out |
| Asset / appliance / warranty tracking | ⬜ | P3 | serials, warranty expiry |
| Recurring service contracts (landscaping, pest) | ⬜ | P3 | |
| Emergency / on-call routing | ⬜ | P3 | |

## 5. Acquisition, disposition & capital projects (flip/invest)

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Deal pipeline / strategy workflows | ✅ | — | rental/flip/BRRRR/hold/wholesale stages; **buy-side `deal` pipeline** (prospecting→owned) + convert-to-property shipped (`docs/DEALS.md`) |
| Underwriting models + sensitivity | ✅ | — | cap rate / cash-on-cash / IRR / DSCR + rent-growth sensitivity, live what-if (`docs/DEALS.md`) |
| Offers / LOIs / purchase contracts | 🟡 | P2 | deal object + offer terms shipped; e-sign of the contract still open |
| Due-diligence checklists + **data room** | ✅ | — | per-deal checklist + document-service data room (`owner_type=deal`) |
| Closing mgmt (title, escrow, settlement) | ⬜ | P3 | title/liens modeled already |
| **Rehab / construction** mgmt (scope, draws, change orders, lien waivers, progress photos) | ✅ | — | `rehab` module: budget + lines + draws (w/ photos) + change orders + generated lien-waiver PDFs (`docs/REHAB.md`) |
| CapEx budgeting + draws | 🟡 | P2 | rehab budget + draws shipped; recurring CapEx reserve still open |
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
| Rent roll, T-12, aging, delinquency reports | ✅ | — | `reports` module: all four off the live ledger + rentals, with CSV/PDF export (`docs/REPORTS.md`) |
| Custom report builder + scheduled exports | ⬜ | P3 | |
| Owner / investor reporting | 🟡 | P2 | owner statements (reconcile w/ payouts) + **1099-NEC/MISC** tax export shipped; K-1s for syndications remain (`docs/REPORTS.md`) |
| Performance analytics + benchmarking | ⬜ | P3 | |
| Map / geospatial portfolio view | ⬜ | P3 | lat/long already enriched |
| Data export / API / warehouse sync | 🟡 | P2 | token API exists |

## 8. Property data & intelligence

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Enrichment: parcel, tax, valuation, schools, utilities, geo | ✅ | — | live geocoder (real county/FIPS) + simulated providers, with graceful fallback |
| Swap simulated → **real** county/AVM/schools APIs | 🟡 | P2 | provider seam + graceful fallback shipped (geocode is live: real county/FIPS); AVM/schools vendors still simulated (`docs/PROPERTY_DATA.md`) |
| Comps / market trends / rent estimates | 🟡 | P3 | AVM exists |
| Hazard/flood/crime/demographics/permits | 🟡 | P3 | flood zone exists |
| **Photos / media / floor plans / virtual tours** | ✅ | — | photos/floorplans in the document store + hero, rendered on the profile (`docs/PROPERTY_DATA.md`) |
| Lease **abstraction** (AI extract terms from PDFs) | ⬜ | P3 | document AI |

## 9. Platform, integrations & cross-cutting

| Feature | Status | Pri | Notes |
|---|---|---|---|
| Multi-tenant + RBAC + workspaces | ✅ | — | runtime-editable roles |
| Pluggable modules (per-tenant on/off) | ✅ | — | |
| White-label theming + **custom domains** | 🟡 | P2 | `custom_domain` field exists |
| Durable background job queue | ✅ | — | retrying, scheduled |
| API **rate limiting** / abuse protection | ✅ | — | fixed-window fairing, tight auth bucket + general bucket, `X-RateLimit-*` + `429`/`Retry-After` (`docs/RATE_LIMITING.md`) |
| **Notifications** (email/SMS/push) + preferences | 🟡 | P1 | auto-email seed |
| **Document storage + e-sign** | ✅ | — | Phase 1 storage + Phase 2 envelopes |
| **Payments** processor + webhooks | ⬜ | P1 | |
| Outbound integration / webhook framework + **secrets/KMS** | ⬜ | P1 | enables every real provider |
| Public/partner API + webhooks + integration marketplace | 🟡 | P2 | scoped token API + signed outbound webhooks (subscribe/replay) shipped |
| **Mobile apps** (manager / resident / inspector, offline) | ⬜ | P3 | |
| **MFA/2FA**, SSO/SAML/SCIM (enterprise) | ⬜ | P2 | |
| Global **search** | ✅ | — | `search` module: command palette across properties/tenants/entities/tickets/LLCs, tenant-scoped + permission-aware |
| **AI copilot** (leasing chat, maintenance triage, comms drafting, report Q&A) | ⬜ | P3 | strong differentiator |
| **Import / migration** (Buildium/AppFolio/Yardi/CSV) | ⬜ | P2 | adoption unlock |
| SaaS **billing/metering** for client workspaces | ✅ | — | per-door metered plans, auto monthly `platform_invoice`, self-serve + HQ console (`docs/SAAS_BILLING.md`) |
| Calendar / scheduling / reminders engine | ✅ | — | `reminder` + per-tenant scan + console calendar (`docs/CALENDAR.md`) |

## 10. Optional verticals (expand TAM)

| Feature | Status | Pri | Notes |
|---|---|---|---|
| **Investor / syndication** suite (cap table, capital calls, waterfalls, distributions, K-1s, investor portal) | 🟡 | P3 | core shipped — commitments, capital calls, distribution waterfalls ([`SYNDICATION.md`](SYNDICATION.md)); K-1s + investor portal remain |
| **HOA / association** mgmt (dues, violations, ARC requests, board/voting) | 🟡 | P3 | core shipped — associations, members, dues, violations, ARC ([`HOA.md`](HOA.md)); board/voting remains |
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
