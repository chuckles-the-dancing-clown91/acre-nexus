# Acre — Multi-Tenant Property-Management Platform

> A "better Zillow" for property-management companies and investors: a full kit
> to list, lease, buy, flip, and manage properties — sold as multi-tenant SaaS,
> with a token-based API so individual services can be resold to other vendors.

This repository contains:

| Path | What it is |
|------|------------|
| `project/`, `chats/` | The original Claude Design prototype + design conversations (the source of truth for look & feel). |
| `backend/` | **Rust** API — Rocket + SeaORM (Postgres) + Tokio. |
| `frontend/` | **Next.js / React** app (App Router, TypeScript, Tailwind). |
| `ARCHITECTURE.md` | How the system is put together and why. |
| `docs/API.md` | REST API reference (auth, tenancy, endpoints, vendor API). |

## What's implemented (this pass)

This is the **foundation + one complete vertical slice**, built to be the proven
pattern the remaining roles plug into:

- **Monorepo** with a Rust workspace (`entity` / `migration` / `api`) and a
  Next.js app.
- **Multi-tenancy** — shared-schema Postgres (`tenant_id` on every scoped row),
  app-layer isolation guards, plus row-level-security policies for defence in depth.
- **AuthN/Z** — JWT access/refresh tokens (Argon2 passwords) for humans, and a
  full **RBAC** system (fine-grained permissions + system roles mirroring the six
  prototype perspectives).
- **Pluggable modules** — the platform is assembled from `PlatformModule`s
  (`properties`, `leasing`, `vendor_api`, `theming`, `flips`) that each own their
  routes, permissions, and background-job kinds. Tenants enable/disable modules
  from settings (`tenant_module` overrides), the scheduler dispatches jobs to the
  owning module, and the frontend renders navigation dynamically from a shared
  registry. Adding a feature is one file + one registry line. See
  `docs/MODULES.md`.
- **Token-based vendor API** — scoped, revocable API keys powering `/api/v1`, so
  services can be sold à la carte.
- **Theming / white-label** — per-tenant branding (logo, colours, legal
  boilerplate templates) driven from the DB; the frontend re-themes at runtime
  plus a dark-mode toggle.
- **Tokio background scheduler** — durable job engine for "progress automation"
  (background checks awaiting a callback, automated emails).
- **Vertical slice UI + API**:
  - **Public website** — branded hero, listings grid, listing detail, working
    application form (which enqueues a screening job).
  - **Landlord / PM console** — portfolio dashboard with live KPIs, properties
    table, **full property profile with computed economics**, LLC-grouped
    portfolio, applications, API-token management, and a staff-only **Platform
    admin** view.

## What's intentionally deferred

The other prototype roles (Tenant portal + 10-step onboarding wizard with dynamic
contract generation, Maintenance work-orders/dispatch, Backoffice ops) are
**designed** in `project/` and have data models ready, but their UIs/endpoints are
not built yet. They follow the same patterns established here.

## Quick start

### 1. Backend

```bash
cd backend
cp .env.example .env                       # adjust DATABASE_URL if needed
createdb acre                              # or use the .env connection
cargo run -p api                           # migrates + seeds + serves :8000
```

Seed creates two demo tenants and three logins (password: `password`):

| Email | Role | Workspace |
|-------|------|-----------|
| `avery@acrehq.com` | Platform staff | Acre HQ (sees all tenants) |
| `jordan@northwind.com` | PM Admin | Northwind Property Group |
| `priya@cascade.com` | PM Admin | Cascade Living LLC |

### 2. Frontend

```bash
cd frontend
cp .env.local.example .env.local
npm install
npm run dev                                # http://localhost:3000
```

Visit `http://localhost:3000` for the public website, or `/login` for the console.

See `ARCHITECTURE.md` and `docs/API.md` for details.
