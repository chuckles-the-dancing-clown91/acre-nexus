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
role; the firm itself can be an owner) and its own **bank accounts**
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

## Invariants (do not regress)

1. **RLS always on** — every new tenant-owned table (`owner`, `entity_ownership`,
   `bank_account`, `portfolio`, `domain`, `onboarding_workflow`) carries
   `tenant_id` + an isolation policy keyed on `app.tenant_id`.
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
