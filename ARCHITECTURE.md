# Architecture

## Overview

```
                    ┌──────────────────────────────────────────┐
   Public visitors  │  Next.js / React frontend (App Router)    │
   Client admins ──▶│  - public website (white-label)           │
   Platform staff   │  - landlord/PM console                    │
                    │  - theming (dark mode + per-tenant brand)  │
                    └───────────────┬──────────────────────────┘
                                    │ HTTPS (JSON)
   Third-party  ───── API key ─────▶│
   vendors                          ▼
                    ┌──────────────────────────────────────────┐
                    │  Rust API — Rocket                         │
                    │  ┌──────────┬───────────┬───────────────┐ │
                    │  │  auth    │  rbac      │  tenancy      │ │
                    │  │ (JWT)    │ (perms)    │ (X-Tenant)    │ │
                    │  ├──────────┴───────────┴───────────────┤ │
                    │  │  routes (public / console / vendor)   │ │
                    │  ├───────────────────────────────────────┤ │
                    │  │  Tokio scheduler (background jobs)     │ │
                    │  └───────────────────────────────────────┘ │
                    └───────────────┬──────────────────────────┘
                                    │ SeaORM
                                    ▼
                          ┌──────────────────┐
                          │   PostgreSQL     │
                          │ shared schema +  │
                          │ tenant_id (+RLS) │
                          └──────────────────┘
```

## Backend (Rust)

A Cargo workspace under `backend/`:

- **`crates/entity`** — SeaORM models. One module per table. Documented; money is
  stored as integer cents (`i64`).
- **`crates/migration`** — schema migrations. `m...init` creates all tables;
  `m...rls` adds Postgres row-level-security policies (defence in depth).
- **`crates/api`** — the Rocket application. Key modules:
  - `config` — env-driven config.
  - `auth` — Argon2 password hashing, JWT issue/verify, the `AuthUser` guard,
    opaque secret hashing for refresh/API tokens.
  - `rbac` — `Permission` enum, the seeded system roles, and a `Grants` set with
    a `platform:admin` super-permission.
  - `tenancy` — `TenantScope` (authenticated; staff can impersonate via
    `X-Tenant`) and `PublicTenant` (resolves a tenant from header/query for the
    unauthenticated website) request guards.
  - `tokens` — minting, hashing, and the `ApiPrincipal` guard for the scoped
    vendor API.
  - `scheduler` — a Tokio task that polls the `background_job` table and advances
    durable state machines (e.g. screening: `pending → awaiting_callback →
    completed`; automated emails). Dispatch is delegated to the owning module.
  - `modules/*` — the **pluggable module system**: each feature area is a
    `PlatformModule` that contributes its routes, the permissions it needs, and
    the background-job kinds it handles. See `docs/MODULES.md`.
  - `routes/*` — handlers grouped by audience (see `docs/API.md`).
  - `openapi` — `rocket_okapi` integration: routes are `#[openapi]`-annotated and
    DTOs derive `JsonSchema`, so the OpenAPI 3.0 doc is **generated from the code**
    and served at `/openapi.json`, with Swagger UI (`/swagger-ui/`) and RapiDoc
    (`/rapidoc/`). Each module contributes its own spec fragment, merged at boot.
  - `error` — one `ApiError` type that serialises to a consistent JSON envelope.

### Why these choices

- **Rocket** for ergonomic, typed request guards — auth, RBAC, and tenant
  resolution compose cleanly as `FromRequest` guards, so every handler signature
  documents its own security requirements.
- **SeaORM** for async, type-safe Postgres access with first-class migrations.
- **Tokio** for the background automation the brief calls for (awaiting
  background-check callbacks, scheduled emails) — jobs are persisted so they
  survive restarts.

## Modularity

The platform is composed of **pluggable modules** so that capabilities can be
shipped, gated, and sold independently. A module declares a stable key, the
permissions it owns, the routes it contributes, and the background-job kinds it
handles; a single `registry()` is the source of truth. The server mounts every
module's routes at boot, the scheduler dispatches jobs to the owning module, and
a `tenant_module` table records per-tenant on/off overrides (falling back to each
module's default). The frontend mirrors the same keys to drive navigation and
settings. Adding a module is a new file plus one registry line — see
`docs/MODULES.md`.

## Multi-tenancy

**Shared schema, `tenant_id` on every scoped row.**

- The application layer is the primary enforcement: every tenant-scoped query
  filters by the active `tenant_id` from the `TenantScope`/`PublicTenant` guard.
- The active tenant comes from: the JWT (`tid`) for client users; the `X-Tenant`
  header for staff impersonation and for the public website; the API token for
  vendor calls.
- Postgres **row-level-security** policies provide a second wall (keyed on a
  `app.tenant_id` session variable). To make RLS bite in production, connect as a
  non-owner DB role and `SET app.tenant_id` per transaction.

## AuthN / AuthZ

- **Humans**: `POST /auth/login` → short-lived JWT **access** token + opaque
  **refresh** token (rotated on `POST /auth/refresh`, revoked on logout). Only
  hashes of refresh tokens are stored.
- **RBAC**: roles bundle fine-grained `resource:action` permissions. The JWT
  embeds the resolved permission set; handlers call `user.require(Permission::…)`.
  Roles → permissions live in the DB, so the Acre dashboard creates roles and
  edits grants at runtime (no redeploy). See **`docs/IAM.md`**.
- **Identity model**: login identity (`app_user`) is separate from the person's
  **profile** (`user_profile`, with SSN/gov-ID encrypted via AES-256-GCM). Users
  hold **memberships** that give them a **persona** (Acre employee vs client
  landlord/back-office/renter…) at platform or tenant scope. Personas, roles, and
  the permission catalog are seeded and editable.
- **Vendors**: long-lived, **scoped**, revocable API keys (`acre_live_…`). Only a
  SHA-256 hash is stored; each `/api/v1` endpoint requires a specific scope so
  services can be sold individually.

## Frontend (Next.js / React)

- **App Router** + TypeScript + Tailwind.
- **Framework stack** (see `frontend/README.md`): TanStack Query for server
  state/caching, React Hook Form + Zod for typed/validated forms, Zustand for
  lightweight global UI state, and **shadcn/ui** (Radix) layered on top of the
  existing design tokens (its CSS variables are bridged to the brand palette, so
  components inherit dark-mode + white-label). Tested with Vitest + Testing
  Library (unit/component) and Playwright (e2e); formatted with Prettier and
  guarded by a Husky + lint-staged pre-commit hook.
- **Design tokens** ported verbatim from the prototype into CSS variables
  (`globals.css`); Tailwind colours reference those variables so the whole palette
  re-themes for dark mode and white-label without a rebuild.
- **`ThemeProvider`** — dark-mode toggle (`.dark` on `<html>`) + per-tenant brand
  (overrides `--accent` at runtime from the tenant theme).
- **`AuthProvider`** — holds the session, hydrates from a stored token, exposes
  `can(permission)` for client-side gating.
- **Modular components** (`components/ui`, `components/*`) — `Card`, `Badge`,
  `Button`, `StatTile`, `ListingCard`, icons, headers — presentational and
  pluggable so new pages compose them.
- **Routes**: `/` (public website), `/listings/[id]` (detail + apply),
  `/login`, and `/console/*` (dashboard, properties + profile, LLCs,
  applications, API tokens, platform admin).

## Extending to the other roles

Each remaining prototype role is additive and follows the established pattern:

1. Add entities + a migration (tenant-scoped).
2. Add tenant-scoped, RBAC-gated routes under `routes/`.
3. Add a console section under `frontend/src/app/console/` reusing `components/ui`.
4. Long-running steps (screening, contract callbacks) become `background_job`s
   advanced by the Tokio scheduler.
