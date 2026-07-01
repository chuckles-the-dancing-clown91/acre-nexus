# Onboarding, Multi-Entity & Tenancy

How Acre Nexus models PM-firm provisioning, multiple LLCs under a firm, the
separate Acre-staff plane, scoped RBAC, white-label multi-domain routing, and
resumable onboarding. This implements the tenancy spec on top of the existing
shared-schema + RLS platform; where the spec proposed a new table that the
codebase already had, we **extended the existing one** rather than fork it.

## Two orthogonal boundaries

| Boundary | What it is | Mechanism |
|---|---|---|
| **Tenant** (`tenant`) | One PM firm — the billing / branding / RLS / RBAC wall | `tenant_id` on every scoped row + Postgres RLS |
| **Legal entity** (`llc`) | An LLC/LP/etc. — the accounting / liability / tax boundary | `entity_id` partitioning of the GL + RBAC scope; **not** an RLS wall |

A firm is one tenant with many legal entities. LLC separation lives in the
accounting and permission layers — not RLS — so firm staff keep consolidated
cross-entity views and "add an LLC" stays an in-app action, never a provisioning
event.

`tenant.parent_org_id` (nullable) is future-proofing for a holding company that
groups several PM-brand tenants for roll-up reporting. Nothing depends on it yet.

## Reconciliation with the existing schema

The spec named some tables the codebase already had under different names. The
implementation maps them as follows:

| Spec concept | Implemented as | Note |
|---|---|---|
| `legal_entities` | the existing `llc` table, enriched | + `entity_type`, `registered_agent`, `status` |
| role `plane` (`platform`/`tenant`) | the existing `role.scope` column | values are exactly `platform` / `tenant` |
| `role_assignments` with a scope | the existing `user_role`, extended | + `scope` + `scope_ref_id` |
| `domains` | new `domain` table | richer than the legacy `tenant.custom_domain` pointer |
| `owners`, `entity_ownership`, `bank_accounts`, `portfolios` | new tables | as specified |
| `platform_staff`, `impersonation_sessions` | new tables | the platform plane |
| `onboarding_workflows` | new `onboarding_workflow` table | one per tenant |

## RBAC: two planes + a scope dimension

**Planes** (the `role.scope` column):

- **platform** — Acre staff (`acre_admin`, `acre_account_manager`, `acre_support`,
  …). These users hold **no tenant membership**.
- **tenant** — firm staff (`tenant_owner`, `property_manager`, `back_office`,
  `leasing_agent`, `maintenance`, `landlord`, `renter`).

**Scope** (on each `user_role` assignment): `platform` | `tenant` | `entity` |
`portfolio` | `property`, plus `scope_ref_id` when narrower than the whole tenant.

Coverage is hierarchical and resolved by **one** centralized function,
`rbac::scope::scope_covers` (`backend/crates/api/src/rbac/scope.rs`):

```
platform ⊇ tenant ⊇ portfolio ⊇ property
                   ⊇ entity (LLC) ⊇ the properties that LLC holds title to
```

- `platform` / `tenant` grants flatten into the JWT permission set
  (`permissions_for`) and cover everything in the workspace.
- `entity` / `portfolio` / `property` grants do **not** flatten; they are checked
  per-resource by `tenancy::resolve::require_scoped`, which builds the resource's
  `ResourceScope` chain and asks whether any scoped assignment that grants the
  permission covers it. So a contract bookkeeper scoped to one LLC can edit only
  that LLC's properties — see `routes/properties/update.rs`.

### Audited impersonation (platform → tenant)

Staff never become members. `POST /platform/impersonate` opens a **time-boxed**
(30 min), **reason-logged**, **revocable** `impersonation_session` and mints a
tenant-scoped access token carrying the staff actor's platform permissions.
Sessions are listable / revocable (`/platform/impersonations`) and every start /
revoke is an audit event.

## Staff assignments (`assignment`)

Firms attach people — property managers, landlords, maintenance, leasing agents,
back-office — to a specific **property** or **legal entity (LLC)**. An assignment
is both a directory relationship *and* an access grant:

- `POST /properties/<id>/assignments` and `POST /entities/<id>/assignments` create
  an `assignment` row (subject_type + subject_id + user + relationship + primary +
  title) **and**, in the same request transaction, a scoped `user_role` grant:
  the relationship's tenant role at `property:{id}` / `entity:{id}` scope. So the
  assignment immediately confers real access via the existing `scope_covers`
  resolver — an LLC assignment covers every property that LLC holds title to.
- `DELETE …/assignments/<id>` removes the row and revokes exactly that grant.
- The grant is idempotent (no duplicate rows) and the whole thing rides the RLS
  request transaction, so a forgotten filter can't leak across tenants.
- Gates reuse existing permissions: `property:write` for property assignments,
  `entity:manage` for LLC assignments; reads need `property:read` / `entity:read`.
- Assignments can be added during onboarding (`POST /properties/onboard` accepts
  an `assignments[]`) or later from the property / LLC detail "Team" card. The
  primary property manager also syncs `property.manager` for display.

Only assignable relationships are the operational tenant roles (`property_manager`,
`landlord`, `maintenance`, `leasing_agent`, `back_office`); `tenant_owner` /
`renter` are deliberately not grantable this way.

## White-label routing (`domain`)

A `domain` maps an inbound `Host` to a **tenant + audience** (`admin` / `owner` /
`renter`). One tenant can map many hosts — `app.firm.com`, `owners.firm.com`,
`pay.firm.com` — each its own audience.

- `GET /public/resolve?host=` (unauthenticated) resolves a host to tenant +
  audience + branding; an unknown/unverified host → 404 (marketing fallback).
- The `PublicTenant` guard falls back to resolving the inbound `Host` against the
  `domain` table and carries the resolved audience.
- Custom domains: `POST /domains` returns a TXT verification token + CNAME/TXT
  DNS instructions; `POST /domains/<id>/verify` records verified + TLS-active.
- **TLS recommendation:** front the app with **Caddy on-demand TLS** keyed to the
  verified `domain` set, rather than hand-rolling ACME — it removes the entire
  cert-renewal surface and satisfies the dependency rule.

## Multi-entity accounting

Each LLC has a **cap table** (`entity_ownership`: owner → basis-point stake →
role; the firm itself can be an owner) — total allocation is capped at 100%
(10000 bps), enforced server-side on add — and its own **bank accounts**
(`operating` / `trust`). The trust **no-commingling invariant** (a posting may
never move funds between two entities' trust ledgers) is enforced in the
accounting domain — `accounting::assert_no_commingling` is the single guard every
future posting must call, with tests. (The double-entry posting engine + ledger
tables are a later milestone; the invariant ships now so it can't be skipped.)

`portfolio` groups properties by investor / strategy / region
(`property.portfolio_id`), orthogonal to which LLC holds title.

## Onboarding state machine (`onboarding_workflow`)

One resumable, audited workflow per tenant. Each step's completion is a predicate
evaluated against the **live database** (`routes/onboarding/state.rs`), so the
checklist never drifts and is resumable from any incomplete step:

```
provisioning → firm_admin_accepted → branding_configured → domains_configured*
  → entities_created → banking_linked → portfolio_imported → staff_invited* → live
```

`*` optional — surfaces as a nudge, doesn't block `live`. `GET /onboarding/workflow`
computes + persists the snapshot; `POST /onboarding/workflow/advance` re-checks
and audits. Provisioning a firm (`POST /platform/provision`) creates the tenant
shell, default theme, reserved `{slug}.acrenexus.com` subdomain, the firm owner
(membership + `tenant_owner` role at tenant scope), and this workflow row.

## System settings (`setting`)

Per-firm configuration lives in a **code-defined catalog** with values stored per
tenant — the same shape as the RBAC/workflow catalogs. The catalog
(`crate::settings::CATALOG`) defines each key's type, default, label, and group;
the `setting` table holds only overrides, so a fresh tenant is fully configured
from defaults and adding a knob never needs a backfill.

- `GET /settings` — the catalog merged with the tenant's effective values.
- `PUT /settings/<key>` `{ value }` — validated (type-checked against the catalog)
  upsert. Both gated by `tenant:manage`; edited on the **Settings** page.
- Typed accessors (`settings::get_bool` / `get_i64`) read a setting inside any
  handler; unknown keys and type mismatches are rejected.

First entries: `application_reuse.enabled` (bool) and `application_reuse.window_days`
(int) — see [LEASING.md](./LEASING.md#reusable-applications-configurable). The
`setting` table is tenant-owned with the same enforced RLS as every other tenant
table.

## Database enforcement: Row-Level Security (the second wall)

Tenant isolation has **two walls**. The **primary wall** is the application: every
tenant-scoped query filters by `tenant_id` (via `TenantScope` / the scope
resolver). The **second wall** is Postgres RLS, which enforces isolation *even if a
handler forgets its filter* — so a bug can't silently leak or cross-write another
tenant's rows.

**How it is enforced** (migration `m20240101_000015_rls_enforce`):

- Every table with a `NOT NULL tenant_id` column gets `ENABLE` **and `FORCE ROW
  LEVEL SECURITY`**. `FORCE` matters: the API connects as the tables' owner, and a
  plain `ENABLE` policy is bypassed for the owner — `FORCE` makes the policy bite
  regardless.
- Each such table carries one policy with **both** `USING` (reads / updates /
  deletes) and `WITH CHECK` (inserts / updated rows), so a row can neither be
  *seen* nor *written* outside the active tenant. The predicate is:

  ```sql
  current_setting('app.tenant_id', true) IS NULL
  OR tenant_id::text = current_setting('app.tenant_id', true)
  ```

- Coverage is discovered **dynamically** from `information_schema` at migration
  time, so it can't drift as new tenant-owned tables are added.

**How the tenant is bound per request** (`backend/crates/api/src/db.rs`): with a
connection pool, `SET LOCAL` is the only way to pin a GUC to the exact connection
running a request. The `RequestDb` guard therefore runs **each request inside one
transaction**, resolves the tenant (JWT `tid` → `X-Tenant` → `Host`), and sets
`app.tenant_id` via `set_config(_, _, true)` (= `SET LOCAL`). It implements
`ConnectionTrait`, so handlers use `&db` exactly like the old `&state.db`; the
`TxCommit` fairing commits on a 2xx response and rolls back otherwise.

**The platform plane is the deliberate exception.** When `app.tenant_id` is unset
(platform staff at Acre HQ, the pre-auth login path, background jobs) the policy's
`IS NULL` branch allows all rows — exactly the cross-tenant access those paths
need. Identity/global tables (`app_user`, `audit_log`, `membership`, `user_role`
— nullable `tenant_id` — and `tenant`, `role`, `role_permission`, `refresh_token`
— no `tenant_id`) are intentionally **excluded** from RLS so login and RBAC keep
working with no tenant context.

## Invariants (do not regress)

1. **RLS on, and enforced** — every tenant-owned table (`NOT NULL tenant_id`)
   carries `ENABLE` + **`FORCE`** RLS and an isolation policy with both `USING`
   and `WITH CHECK` keyed on `app.tenant_id`, set per request via `SET LOCAL` by
   the `RequestDb` transaction guard (see *Database enforcement* above). Nullable-
   /absent-`tenant_id` identity tables stay excluded.
2. **Trust accounting: no commingling** — enforced in `accounting`, not just UI.
3. **Platform staff are not tenant members** — tenant access only via an
   `impersonation_session` (time-boxed, reason-logged, revocable).
4. **Audit on every mutation** — provisioning, impersonation, domains, cap-table,
   banking, onboarding, and scoped role grants all emit domain events.
5. **Scope coverage is centralized** — one `scope_covers()` resolver; handlers
   never scatter ad-hoc scope checks.
6. **No auto-migrate in prod** — migrations remain an explicit gated step
   (`AUTO_MIGRATE`); this milestone does not change that.

## Answers to the spec's open questions (§13)

1. **Self-serve vs Acre-provisioned** — implemented Acre-provisioned first
   (`POST /platform/provision`, gated by `tenant:manage`). A public signup path
   can reuse the same provisioning transaction when desired.
2. **Enrichment source** — kept behind the existing `enrichment` provider port,
   so providers remain swappable (dependency rule).
3. **Owner/renter portal in v1** — the `audience` dimension and portal roles ship
   now (admin/owner/renter domains resolve), so portals are additive later.
4. **Caddy vs in-process ACME** — recommended Caddy on-demand TLS; the API only
   records the verified `domain` set it keys off.
5. **Cross-tenant holding company** — `tenant.parent_org_id` lands as nullable
   future-proofing; no roll-up code yet.

## Build map (where each phase lives)

- **A — entity model:** `migration/m20240101_000010_tenancy_entities.rs`;
  entities `owner`, `entity_ownership`, `bank_account`, `portfolio`; `rbac/scope.rs`.
- **B — platform plane:** `m..._000011_platform_plane.rs`; `routes/platform/{impersonate,impersonations,staff}.rs`.
- **C — provisioning:** `routes/platform/provision.rs`; scoped `routes/iam/assign_role.rs`.
- **D — routing:** `m..._000012_domains_onboarding.rs`; `routes/domains/*`; `modules/domains.rs`.
- **E — portfolio/banking/onboarding:** `routes/{portfolios,cap_table,banking}/*`; `accounting.rs`; `routes/onboarding/{state,workflow}.rs`.
- **F — scoped RBAC + UX:** `tenancy/resolve.rs`; `frontend` console `domains` + `onboarding` pages.
