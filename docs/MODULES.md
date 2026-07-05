# Pluggable Modules

Acre Nexus is assembled from self-contained **modules**. A module owns a slice of
the product — its routes, the permissions it requires, the background-job kinds it
processes, and whether it ships on by default. Tenants turn modules on and off
from their software settings, which makes capabilities **sellable per feature**.

The backend and frontend share a module **key** (`properties`, `property_intel`,
`leasing`, `vendor_api`, `theming`, `flips`) so the two halves always agree on
what a module is and how it is gated.

## Modules today

| Key | Name | Default | Routes / responsibility |
|-----|------|---------|--------------------------|
| `properties` | Property Management | on | `/properties`, `/properties/onboard`, `/portfolio`, `/llcs`, `/properties/{id}/mortgages`, `/properties/{id}/workflow` (see `docs/INVESTING.md`) |
| `property_intel` | Property Intelligence | on | `/properties/{id}/intel`, `/enrich`, `/enrichment` + the `enrich_*` enrichment jobs (see `docs/PROPERTY_DATA.md`) |
| `entities` | Entities & Contacts | on | `/entities` registry (banks, lenders, contractors …) + notes (see `docs/INVESTING.md`) |
| `rentals` | Rentals & Leasing | on | `/properties/{id}/units`, `/leases`, `/leases/{id}/payments` (see `docs/RENTALS.md`) |
| `accounting` | Accounting & Payments | on | `/accounting/*` (ledger + reports), `/payments`, `/my/lease` + `/my/payments` + `/my/payment-methods` + `/my/autopay`, `/bank-accounts/*` + `/bank-transactions/*`, `/payouts`, `/finance/series` + the `billing_cycle`/`payment_process`/`bank_feed_sync`/`payout_execute` jobs (see `docs/PAYMENTS.md`) |
| `maintenance` | Maintenance & Work Orders | on | `/tickets`, `/properties/{id}/tickets`, `/tickets/{id}/comments` (see `docs/RENTALS.md`) |
| `title` | Title & Ownership | on | `/properties/{id}/ownership`, `/properties/{id}/liens` (see `docs/RENTALS.md`) |
| `leasing` | Leasing & Listings | on | `/public/*`, `/applications` + the screening jobs |
| `vendor_api` | Vendor API | on | `/api-tokens`, `/api/v1/*` |
| `theming` | Branding & Theming | on | `/theme` |
| `integrations` | Integrations | on | `/integrations/secrets`, `/integrations/providers`, `/integrations/notifications`, `/notifications/*` (inbox + web push), `/documents`, `/webhooks/{provider}` + the `auto_email`/`auto_sms`/`auto_push`/`auto_chat`/`webhook_event`/`document_retention` jobs (see `docs/INTEGRATIONS.md`, `docs/NOTIFICATIONS.md`) |
| `flips` | Acquisitions & Flips | **off (preview)** | `/modules/flips/pipeline` |

## Backend contract

Each module is a unit struct implementing `PlatformModule`
(`backend/crates/api/src/modules/`):

```rust
#[rocket::async_trait]
pub trait PlatformModule: Send + Sync {
    fn manifest(&self) -> ModuleManifest;          // key, name, permissions, job_kinds, default_enabled, preview
    fn routes(&self) -> Vec<rocket::Route> { vec![] }
    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> { None }
}
```

The single `registry()` lists every module. From there the platform wires itself:

- **Routing** — `main.rs` mounts `routes::core()` (health, auth, platform admin,
  module management) and then every module's `routes()`.
- **Scheduling** — `scheduler.rs` routes each due `background_job` to the module
  that declares its `kind` in `job_kinds`, calling `handle_job`. If the owning
  tenant has the module disabled, the job is *parked* (rescheduled, no attempt
  consumed) until it is re-enabled.
- **Enablement** — `is_enabled(db, tenant_id, key)` resolves the `tenant_module`
  override table, falling back to the module's `default_enabled`. Optional
  modules call `require_enabled(...)` in their routes to self-gate (see
  `modules/flips.rs`).

### Adding a backend module

1. Create `modules/<name>.rs` with a struct implementing `PlatformModule`.
2. Add one line to `registry()`.

Nothing else needs to change — routing, scheduling, and the `/modules` settings
API pick it up automatically.

## Tenant module API

Gated by `tenant:manage`.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/modules` | Every module with its resolved `enabled` state for the active tenant |
| PATCH | `/modules/{key}` | `{ "enabled": bool }` — upsert the tenant's override (404 for unknown keys) |

Disabling a module hides its navigation, makes its self-gated routes return
`403 module 'x' is not enabled for this tenant`, and parks its background jobs.

## Frontend contract

`frontend/src/modules/registry.ts` mirrors the backend keys and declares each
module's navigation entries (with the permission that gates them). At runtime:

- `ModulesProvider` (`lib/modules.tsx`) fetches `/modules` and exposes
  `isEnabled(key)` and `setEnabled(key, on)`; it falls back to registry defaults
  when the caller can't read the config.
- The console sidebar renders dynamically: a nav item shows only when its module
  is **enabled** *and* the user holds the item's permission.
- The **Modules** settings screen (`/console/modules`) toggles modules live.
- Module pages can code-split their heavy parts with `next/dynamic` — see the
  `flips` page, whose deal board is loaded lazily so optional modules don't weigh
  down the core bundle.

## The `flips` example

`flips` is the reference module: shipped as a **preview** (off by default), it
owns its permissions, contributes one self-gating route, and adds a lazily-loaded
console page. Promoting it to GA is a one-line manifest change
(`preview: false`, `default_enabled: true`); building out the domain means adding
a `deal` entity + migration and richer routes — with no changes anywhere else.
