# Acre Nexus

[![CI](https://github.com/chuckles-the-dancing-clown91/acre-nexus/actions/workflows/ci.yml/badge.svg)](https://github.com/chuckles-the-dancing-clown91/acre-nexus/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/backend-Rust%20%2B%20Rocket-orange)
![Next.js](https://img.shields.io/badge/frontend-Next.js%2015%20%2B%20React%2019-black)
![PostgreSQL](https://img.shields.io/badge/database-PostgreSQL-336791)
![License](https://img.shields.io/badge/license-Proprietary-lightgrey)

A multi-tenant **property-management and real-estate investment platform**.
Acre Nexus gives property-management firms a white-label back office and public
leasing site, and gives investor-operators the acquisition, financing, and
portfolio tooling that incumbent PM software treats as an afterthought.

## Features

- **Multi-tenant core** — shared-schema tenancy with application-layer guards
  *and* enforced Postgres row-level security; per-tenant white-label domains,
  branding, and legal templates.
- **IAM & RBAC** — JWT auth, fine-grained permissions, seeded system roles and
  personas, and field-level PII encryption (AES-256-GCM).
- **Audit everywhere** — every HTTP request is access-logged, and every state
  change (including background-job and pipeline mutations: screening verdicts,
  lease activation, listing sync, signed-PDF stores) writes a domain audit
  event with a stable action taxonomy — see [`docs/AUDIT.md`](docs/AUDIT.md).
- **Pluggable modules** — each feature area (properties, rentals, leasing,
  maintenance, title, flips, integrations …) is a self-contained module a
  tenant can toggle from settings; adding one is "a file plus a registry line".
- **Property management** — portfolio and LLC holding entities, property
  profiles, units, leases and the rent ledger, conditional fee schedules,
  templated lease documents, maintenance work orders, and tenant history.
- **Property intelligence** — automated enrichment (geocoding, parcel, tax,
  valuation, schools, utilities) behind a provider interface with deterministic
  simulated sources and a live geocoder.
- **Acquisitions & underwriting** — a buy-side **deal pipeline** (prospecting →
  offer → under contract → closing → owned) with investor-grade underwriting
  (cap rate, cash-on-cash, IRR, DSCR + rent-growth sensitivity, live what-if), a
  due-diligence checklist and document data room, and one-click conversion of a
  closed deal into a fully-onboarded property — see [`docs/DEALS.md`](docs/DEALS.md).
- **Leasing funnel** — end to end: public listings site, three application
  doors (anonymous website, white-glove renter portal that auto-fills from the
  tenant's profile, back office), a settings-driven screening pipeline
  (credit floor / income multiple, optional auto-approve), one-click
  application→lease conversion with auto-applied fees and a generated lease
  document, and automatic listing/occupancy/workflow sync at every step.
- **Tenant screening (FCRA)** — real consumer reports (credit + criminal +
  eviction) through a sandbox-first Checkr provider: §604(b) consent captured
  at every intake door, a stored `screening_report` per application behind its
  own `screening:read` permission, policy verdicts landing on the application,
  and a §615(a) adverse-action workflow — notice generated, filed as a PDF,
  emailed, and audited (auto-send on decline or one click from the console).
- **E-signature** — native envelope flow with tokenized signing links (email +
  SMS), reminders that keep the original links working, an ESIGN/UETA audit
  trail (typed signature, consent, IP, user agent, pinned body hash), signed
  PDF stored in the document service, and in-person signing as a first-class
  alternative — race-proof against simultaneous final signatures.
- **Accounting core** — a double-entry general ledger with a chart of
  accounts per LLC, one validated posting path (every transaction balances,
  so trial balances sum to zero), trust/escrow accounting with the
  no-commingling invariant enforced at posting time, and trial balance /
  income statement / trust reconciliation reports.
- **Payments** — rent, deposits, and fees collected by card/ACH through
  Stripe (sandbox-first; tokenized saved methods, never PANs) with autopay,
  webhook-driven settlement, automatic receipts, and a settings-driven
  late-fee engine; a recurring billing cycle raises receivables and keeps
  the books current.
- **Bank feeds & payouts** — Plaid-linked accounts sync transactions and
  auto-match deposits against settled payments (manual match/ignore for the
  rest); owner payouts compute from the ledger (rent − expenses − management
  fee), execute as ACH, and file a generated owner statement.
- **Financial dashboards** — 12-month trends (rent collected, occupancy,
  delinquency, NOI, portfolio value) from live ledger rollups + monthly
  snapshots, charted with a dependency-free SVG component; residents get a
  full pay-rent portal (balance, one-click pay, methods, autopay, receipts).
- **Per-tenant settings** — a code-defined catalog of workspace knobs
  (screening policy, signing-link expiry, signer caps, document retention and
  titles, application reuse, auto-approve …) editable from the console, each
  change audited.
- **Integration substrate** — encrypted credential vault, typed outbound
  provider framework, signature-verified inbound webhooks, and S3-compatible
  document storage with signed URLs and versioning — all riding a durable
  background-job queue with retries.
- **Notifications** — templated email, SMS, browser Web Push (VAPID +
  RFC 8291), Slack/Discord chat, and a per-user in-app inbox with unread
  badges; tenants connect their own delivery providers (Resend, SendGrid,
  Postmark, Twilio) from the console, with simulated senders as the default.
- **Vendor API** — scoped, revocable API tokens and a public `/api/v1` surface,
  documented via OpenAPI (Swagger UI + RapiDoc).

## Architecture

```
┌────────────────────────┐         ┌─────────────────────────────┐
│  frontend/  (Next.js)  │  HTTPS  │  backend/   (Rust)          │
│  console + public site │ ──────► │  Rocket API + OpenAPI       │
│  Tailwind, TanStack    │         │  SeaORM ► PostgreSQL (RLS)  │
└────────────────────────┘         │  Tokio job scheduler        │
                                   │  Pluggable platform modules │
                                   └─────────────────────────────┘
```

- **Backend** — Rust workspace (`api`, `entity`, `migration` crates) on Rocket,
  SeaORM, and Tokio. Every module contributes routes, permissions, background
  job kinds, and an OpenAPI fragment.
- **Frontend** — Next.js App Router + React + TypeScript + Tailwind CSS, with a
  module registry mirroring the backend so navigation and gating stay in sync.
- **Database** — PostgreSQL with per-tenant row-level security as defence in
  depth behind application-layer tenancy guards.

## Getting started

### Prerequisites

- Rust (stable, 1.94+)
- Node.js 22+
- PostgreSQL 14+

### Backend

```bash
cd backend
cp .env.example .env          # set DATABASE_URL, JWT_SECRET, …
cargo run -p api              # migrates, seeds demo data, serves on :8000
```

Interactive API docs are served at `http://localhost:8000/swagger-ui` and
`/rapidoc`. Demo logins are seeded (see `backend/README.md`); all demo users
share the password `password`.

### Frontend

```bash
cd frontend
npm install
cp .env.local.example .env.local   # points at http://localhost:8000 by default
npm run dev                        # serves on :3000
```

### Tests & checks

```bash
# backend
cd backend && cargo fmt --all -- --check && cargo clippy --workspace --all-targets && cargo test --workspace

# frontend
cd frontend && npm run lint && npm run typecheck && npm run test && npm run build
```

CI runs the same suite on every push and pull request.

## Repository layout

| Path | Contents |
| --- | --- |
| `backend/` | Rust workspace: `crates/api` (Rocket app), `crates/entity` (SeaORM models), `crates/migration` (schema + RLS) |
| `frontend/` | Next.js app: console, public leasing site, module registry |
| `docs/` | Deep-dive documentation (see below) |
| `project/` | Original HTML design prototypes the UI was built from |
| `ARCHITECTURE.md` | System design overview |
| `IMPLEMENTATION.md` | Implementation notes and conventions |

## Documentation

| Doc | Topic |
| --- | --- |
| [`docs/PRODUCT.md`](docs/PRODUCT.md) | Product vision and personas |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | System architecture |
| [`docs/TENANCY.md`](docs/TENANCY.md) | Multi-tenancy, domains, provisioning |
| [`docs/IAM.md`](docs/IAM.md) | Users, roles, permissions, PII |
| [`docs/MODULES.md`](docs/MODULES.md) | The pluggable module system |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Secrets vault, providers, webhooks, documents |
| [`docs/NOTIFICATIONS.md`](docs/NOTIFICATIONS.md) | Email/SMS/push/chat delivery, providers, in-app inbox |
| [`docs/PROPERTY_DATA.md`](docs/PROPERTY_DATA.md) | Property intelligence & enrichment |
| [`docs/RENTALS.md`](docs/RENTALS.md) | Units, leases, ledger, maintenance, title |
| [`docs/PAYMENTS.md`](docs/PAYMENTS.md) | Ledger, payments, late fees, bank feeds, payouts, dashboards |
| [`docs/LEASING.md`](docs/LEASING.md) | Listings, applications, screening |
| [`docs/SCREENING.md`](docs/SCREENING.md) | FCRA screening, consent, adverse action |
| [`docs/INVESTING.md`](docs/INVESTING.md) | Entities, financing, workflows |
| [`docs/AUDIT.md`](docs/AUDIT.md) | Audit trail & logging |
| [`docs/API.md`](docs/API.md) | API conventions & vendor tokens |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Phased roadmap to GA |

## Roadmap

Development is tracked in [GitHub issues](https://github.com/chuckles-the-dancing-clown91/acre-nexus/issues)
against the [roadmap](docs/ROADMAP.md) (which carries the living **TODO**
list): foundation + integration substrate (Phases 0–1, shipped), documents &
e-signature with the full listing→application→screening→lease pipeline
(Phase 2, shipped and hardened), payments + accounting core + financial
dashboards (Phase 3, shipped: double-entry ledger, Stripe/Plaid sandbox,
autopay, late fees, payouts, trends), FCRA tenant screening (Phase 4,
shipped: Checkr sandbox, consent, screening reports, adverse action),
tenant lifecycle / resident portal (partial — conversion, the white-glove
portal, and rent payment shipped), then helpdesk, real data providers, and
reporting/GA.

## Topics

`property-management` · `real-estate` · `proptech` · `saas` · `multi-tenant` ·
`rust` · `rocket` · `sea-orm` · `postgresql` · `nextjs` · `react` ·
`typescript` · `tailwindcss` · `rest-api` · `openapi`

## License

Proprietary — all rights reserved.
