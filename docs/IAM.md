# Identity & Access Management

Acre Nexus separates **who you are**, **what you are**, and **what you can do**.

```
 app_user ──1:1── user_profile            (identity)        (PII, SSN/ID encrypted)
    │
    ├── membership ──→ profile_type        (persona: Acre employee | tenant landlord/…)
    │       (scope = platform | tenant)
    │
    └── user_role ──→ role ──→ role_permission ──→ permission   (RBAC, editable at runtime)
```

## Layers

| Concept | Table | Meaning |
|---------|-------|---------|
| **User** | `app_user` | Bare login: email, optional username, password, `status`. |
| **Profile** | `user_profile` | 1:1 PII — legal name, DOB, address, **SSN & gov-ID (encrypted)**. |
| **Persona** | `membership` → `profile_type` | What kind of actor a user is in a scope. |
| **Role** | `role` + `role_permission` | A named, editable bundle of permissions. |
| **Permission** | `permission` catalog | A capability key (`property:read`, `role:manage`, …). |
| **Assignment** | `user_role` | Grants a role to a user (optionally tenant-scoped). |

A user can hold **multiple memberships** — e.g. an Acre support agent who is also
a landlord inside a client workspace. Each membership has a persona; permissions
come from the roles assigned to the user.

## Personas (`profile_type`)

**Platform (Acre HQ employees):** `acre_admin`, `acre_account_manager`,
`acre_support`, `acre_billing`, `acre_read_only`.

**Tenant (client workspace):** `tenant_owner`, `property_manager`, `back_office`,
`leasing_agent`, `maintenance`, `landlord`, `renter`.

Each persona has a **default role** granted automatically when a member is
created with it. Personas and their catalogs are seeded but extensible.

## Permissions

Permissions are `resource:action` strings, resolved per user at login and
embedded in the JWT. The catalog (seeded from `rbac::PERMISSION_CATALOG`) covers
properties, leasing, billing, settings, and the IAM controls:
`user:read/manage`, `profile:read/write/read_pii`, `member:read/manage`,
`role:read/manage`, plus the `platform:admin` super-permission (implies all).

Because roles → permissions live in the database, **the Acre dashboard creates
roles and edits permission grants at runtime** — no redeploy. Custom permissions
can be appended to the catalog for new modules.

## Sensitive PII

SSN and government-ID numbers are encrypted with **AES-256-GCM** (`api::pii`)
under `PII_ENC_KEY` (or a key derived from `JWT_SECRET` in dev). Only base64
ciphertext + a per-value nonce and the **last four digits** are stored. The
plaintext is returned only via `GET /admin/users/{id}/pii`, gated by the
dedicated `profile:read_pii` permission and logged as an access event.

> Production: back `PII_ENC_KEY` with a KMS/HSM, rotate it, and ship the PII
> access log to an audit sink.

## API surface (all under the generated OpenAPI docs, tag **IAM**)

Acre admin (`/admin/*`, platform staff / `user:*`, `role:*`, `profile:*`):

| Method | Path | Permission |
|--------|------|-----------|
| GET | `/admin/permissions` | `role:read` |
| GET | `/admin/profile-types` | `member:read` |
| GET/POST | `/admin/roles` | `role:read` / `role:manage` |
| PATCH/DELETE | `/admin/roles/{id}` | `role:manage` |
| GET/POST | `/admin/users` | `user:read` / `user:manage` |
| GET/PATCH | `/admin/users/{id}` | `user:read` / `user:manage` |
| PUT | `/admin/users/{id}/profile` | `profile:write` |
| GET | `/admin/users/{id}/pii` | `profile:read_pii` |
| POST | `/admin/users/{id}/memberships` | `member:manage` |
| DELETE | `/admin/memberships/{id}` | `member:manage` |
| POST | `/admin/users/{id}/roles` | `role:manage` |
| DELETE | `/admin/user-roles/{id}` | `role:manage` |

Client admins (tenant-scoped, `/members`):

| Method | Path | Permission |
|--------|------|-----------|
| GET | `/members` | `member:read` |
| POST | `/members` | `member:manage` |

## Demo logins (password `password`)

| Email | Persona | Workspace |
|-------|---------|-----------|
| `avery@acrehq.com` | Acre Admin | Platform |
| `sam@acrehq.com` | Support Agent | Platform |
| `jordan@northwind.com` | Workspace Owner | Northwind |
| `morgan@northwind.com` | Back-office | Northwind |
| `lee@northwind.com` | Landlord | Northwind |
| `priya@cascade.com` | Workspace Owner | Cascade |

## Workspaces & switching (multi-membership users)

Because a user can hold memberships in several workspaces, the session is scoped
to one **active workspace** at a time, and permissions are resolved for it:

- The JWT carries the active `tid`; `permissions_for(user, active_tenant)`
  includes a role assignment when it is platform-scoped (`tenant_id IS NULL`,
  always) **or** matches the active workspace. So switching workspace changes the
  effective permission set.
- `GET /auth/me` returns the user's `memberships` (with resolved tenant
  name/slug), the `workspaces` they can enter, and the `active_tenant_id`.
- `GET /auth/workspaces` lists switchable workspaces.
- `POST /auth/switch { tenant_id? }` re-issues an access token scoped to the
  chosen workspace (`null` = Acre HQ / platform). Non-staff must hold an active
  membership in the target; staff may enter any (the `TenantScope` guard treats a
  switched staff session as impersonation). The refresh token is unchanged.

The `TenantScope` request guard honors the active `tid` first; staff with no
active workspace can still impersonate via the legacy `X-Tenant` header.

## Audit log

`audit_log` now captures activity at two levels (best-effort; an audit write
never fails the underlying request): **every request** is recorded by a fairing
(`http.request`, with method/path/status/latency/principal + an `X-Request-Id`),
and every state change emits a semantic **domain event** (`pii.reveal`,
`user.create`, `role.{create,update,delete}`, plus property/llc/application/theme/
module/token/auth events across the rest of the platform). Each row captures the
actor, action, target, workspace, and optional metadata.

`GET /admin/audit?limit=&action=` returns recent entries (newest first, actor
name resolved), gated by the `audit:read` permission (held by Acre admin,
account-manager, and read-only roles). Ship this table to an external,
append-only audit sink in production. Full design lives in **`docs/AUDIT.md`**.
