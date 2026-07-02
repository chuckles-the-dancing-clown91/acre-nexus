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
  personas, field-level PII encryption (AES-256-GCM), and a full audit trail of
  every state change.
- **Pluggable modules** — each feature area (properties, rentals, leasing,
  maintenance, title, flips, integrations …) is a self-contained module a
  tenant can toggle from settings; adding one is "a file plus a registry line".
- **Property management** — portfolio and LLC holding entities, property
  profiles, units, leases and the rent ledger, conditional fee schedules,
  templated lease documents, maintenance work orders, and tenant history.
- **Property intelligence** — automated enrichment (geocoding, parcel, tax,
  valuation, schools, utilities) behind a provider interface with deterministic
  simulated sources and a live geocoder.
- **Leasing funnel** — public listings site, applications, screening pipeline,
  and an auditable application workflow.
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
| [`docs/LEASING.md`](docs/LEASING.md) | Listings, applications, screening |
| [`docs/INVESTING.md`](docs/INVESTING.md) | Entities, financing, workflows |
| [`docs/AUDIT.md`](docs/AUDIT.md) | Audit trail & logging |
| [`docs/API.md`](docs/API.md) | API conventions & vendor tokens |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Phased roadmap to GA |

## Roadmap

Development is tracked in [GitHub issues](https://github.com/chuckles-the-dancing-clown91/acre-nexus/issues)
against the [roadmap](docs/ROADMAP.md): hardening (T0), integration substrate
(Phase 1, shipped), documents & e-signature (Phase 2, shipped), payments +
accounting core, screening, resident portal, helpdesk, real data providers,
and reporting/GA.

## Topics

`property-management` · `real-estate` · `proptech` · `saas` · `multi-tenant` ·
`rust` · `rocket` · `sea-orm` · `postgresql` · `nextjs` · `react` ·
`typescript` · `tailwindcss` · `rest-api` · `openapi`

## License

Proprietary — all rights reserved.
