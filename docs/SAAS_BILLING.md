# SaaS platform billing

Acre Nexus is a multi-tenant SaaS: client property-management firms ("workspaces")
subscribe to the platform. **SaaS billing** is the revenue side — Acre HQ metering
each workspace's usage and billing it monthly. This is distinct from the *resident*
rent-billing cycle (see [`PAYMENTS.md`](PAYMENTS.md)); that bills renters on a
workspace's behalf, while this bills the workspace itself.

Roadmap Phase 8. Backend engine: `backend/crates/api/src/saas.rs`.

## Why it isn't a module

Every product feature is a [pluggable module](MODULES.md) a tenant can switch off.
Billing is the one thing they can't — a workspace can't disable being invoiced.
So SaaS billing is **core infrastructure**, wired directly into the boot sequence
and the scheduler (like the resident billing cycle in `billing.rs`), never listed
in `modules::registry()`.

## Plans

Three published plans, priced per-door: a monthly base fee that includes a unit
allowance, then a metered overage on **units under management** beyond it. The
catalogue lives in `saas::PLANS` (pure Rust, unit-tested).

| Plan | Base / mo | Included units | Overage / unit |
|------|-----------|----------------|----------------|
| **Starter** | $49 | 25 | $2.50 |
| **Growth** | $199 | 100 | $2.00 |
| **Enterprise** | $799 | 500 | $1.50 |

A tenant's current plan is `tenant.plan`; unknown / legacy values fall back to
Starter. Pricing is frozen onto each invoice's line items at generation time, so
a bill stays reproducible even if the catalogue later changes.

## Metering

`saas::meter(db, tenant)` counts a workspace's **units under management** (the sum
of `property.units`) — the billable quantity — plus its property count for context.
The overage is `max(0, units − included) × overage_per_unit`.

## Invoices

`platform_invoice` — one bill per tenant per billing month (unique on
`(tenant_id, period)`, so generation is idempotent) — with its frozen
`platform_invoice_line`s. Both are tenant-scoped with enforced RLS; the platform
plane (staff, null tenant GUC) authors them across every workspace.

Lifecycle: `open` → `paid` (settled) or `void` (written off). Generated invoices
are issued immediately with a net-15 due date.

### Automatic monthly run

Each tenant has a self-rescheduling `platform_billing` background job (ensured at
boot and on provisioning). Roughly daily it makes sure the **previous month's**
invoice exists for its workspace — skipping any period that predates the
workspace's creation — then sleeps. Because it's core (not a module), the
scheduler dispatches it directly and never parks it on module enablement.

## API

### Workspace self-serve — gated by `billing:read`

| Method | Path | Description |
|--------|------|-------------|
| GET | `/billing/subscription` | Current plan, live meter, this-period estimate, outstanding balance, and the plan catalogue |
| GET | `/billing/invoices` | This workspace's invoices, newest first |
| GET | `/billing/invoices/{id}` | One invoice with its line items |
| GET | `/billing/invoices/{id}/export?format=csv\|pdf` | Downloadable invoice |

RLS scopes every self-serve query to the caller's workspace.

### Platform plane — gated by `platform:admin` (staff, cross-tenant)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/platform/billing/overview` | Platform MRR + per-workspace plan, usage, and outstanding balance |
| GET | `/platform/billing/invoices?status=&period=` | The full invoice ledger, filterable |
| POST | `/platform/billing/run` | Generate invoices for a period (default: previous month) across every tenant |
| POST | `/platform/billing/invoices/{id}/pay` | Mark an open invoice paid |
| POST | `/platform/billing/invoices/{id}/void` | Void an invoice |
| PATCH | `/platform/billing/tenants/{id}/plan` | Move a workspace to a different plan |

Plan changes, billing runs, and settlements are all audit-logged.

## Console

- **Workspace → Billing** (`/console/billing`, `billing:read`): plan card + live
  meter, the estimated charge for the period in progress broken down by line, a
  plan comparison, and downloadable invoice history.
- **Platform admin → Billing** (`/console/platform/billing`, staff): MRR / outstanding
  tiles, a one-click billing run, per-workspace plan management, and the invoice
  ledger with settle / void actions.
