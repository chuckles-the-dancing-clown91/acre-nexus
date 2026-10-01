# Who sees what, and whose brand

## Reach: which properties someone sees

Permissions say **what** someone may do (`property:read`,
`maintenance:manage`). Reach says **where**.

| Persona | Reach |
|---|---|
| Company owner (`tenant_owner`), back office | The whole company |
| Property manager, leasing agent, maintenance | Properties assigned to them, directly or through an assigned LLC |
| Owner (`landlord`) | Properties of the LLCs they are assigned to (or assigned directly) |
| Vantedge staff (support, impersonation) | The whole company they are viewing |

A person holding any company-wide persona in a workspace has company reach
there. Assignments are made on a property's or LLC's People panel, by the
company.

### How it is enforced

`backend/crates/api/src/tenancy/access.rs`:

- `Access` (a request guard) is computed once per request from the person's
  memberships and assignments.
- A central gate runs inside `RequestDb`, before any data route. For a
  property-scoped person it allows only:
  - routes with no property data (sign-in, their own profile, notifications,
    the module list);
  - list routes that narrow their own results by reach: `/properties`,
    `/portfolio/summary`, `/portfolio/llcs`, `/search`, `/tickets`, `/leases`;
  - routes addressed by one property, ticket, lease or unit within reach
    (`/properties/<id>/…`, `/tickets/<id>/…`, `/leases/<id>/…`,
    `/units/<id>/…`). Out of reach answers **404**, the same as another
    company's data.

  Everything else answers **403**. A new route is closed to scoped people until
  it is made reach-aware, never open by accident. Assigning people and deleting
  a property stay with the company even on an assigned property.
- `GET /auth/me` returns `reach: { scope: "company" | "properties",
  property_ids }`. The console uses it to offer only screens that work
  (`REACH_AWARE` in `frontend/src/components/shell/nav.ts`).

### Making another area reach-aware

1. In the list handler, take `access: crate::tenancy::Access` and narrow the
   query with `access.property_ids()` (or filter rows with `access.sees`).
2. Add the route to `SELF_FILTERED` in `access.rs`.
3. Add its console path to `REACH_AWARE` in `nav.ts`.
4. Extend the `property_reach` integration scenario.

Still company-only today: payments, accounting, applications, listings,
documents outside a property, reports, calendar, messages and texts.

## Branding: whose name is on the screen

Decided by the host, in `frontend/src/theme/gate.ts`:

| Host | Brand |
|---|---|
| A client's verified domain (any audience) | The client's name, logo and colour, "Powered by Vantedge" |
| Vantedge's hosts (`PLATFORM_HOSTS`) and localhost | Vantedge |
| Anything else | Vantedge, public surface |

On Vantedge's hosts a client workspace appears as a monogram of its name, and
Vantedge HQ always shows the Vantedge mark, so back-office and HQ work never
carries a client's branding. Set `BRANDED_TENANT_PREVIEW=<slug>` locally to see
a client's branded console.
