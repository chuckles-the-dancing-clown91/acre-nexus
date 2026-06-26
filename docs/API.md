# Acre API Reference

Base URL (dev): `http://localhost:8000`

## Interactive / machine-readable docs

The spec below is also **auto-generated from the Rust code** with `rocket_okapi`
(`#[openapi]` routes + `JsonSchema`-deriving DTOs). When the server is running:

| URL | What |
|-----|------|
| `GET /openapi.json` | The full OpenAPI 3.0 document (generated, always in sync with the routes) |
| `/swagger-ui/` | Swagger UI explorer |
| `/rapidoc/` | RapiDoc explorer |

This hand-written reference mirrors it for convenience; the generated
`openapi.json` is the source of truth (security schemes `jwt` / `api_key`,
request/response schemas, and tags per area are all derived from the handlers).

All responses are JSON. Errors use a consistent envelope:

```json
{ "error": { "code": "forbidden", "message": "missing permission: property:write" } }
```

## Authentication

Two independent auth schemes:

| Scheme | Header | Used by |
|--------|--------|---------|
| **JWT** (human users) | `Authorization: Bearer <access_token>` | Web console |
| **API key** (vendors) | `Authorization: Bearer acre_live_…` or `X-Api-Key: acre_live_…` | `/api/v1/*` |

### Tenant resolution

| Caller | Source of tenant |
|--------|------------------|
| Client user | `tenant_id` claim in their JWT |
| Platform staff | `X-Tenant: <slug|uuid>` header ("view as client") |
| Public website | `X-Tenant` header or `?tenant=<slug>` |
| Vendor | the API token's tenant |

---

## Auth endpoints

### `POST /auth/login`
```json
// request
{ "email": "jordan@northwind.com", "password": "password" }
// response
{ "access_token": "...", "refresh_token": "...", "token_type": "Bearer",
  "expires_in": 900, "user": { "id": "...", "email": "...", "name": "...",
  "tenant_id": "...", "is_platform_staff": false, "permissions": ["property:read", ...] } }
```

### `POST /auth/refresh`
`{ "refresh_token": "..." }` → new token pair (old refresh token is rotated/revoked).

### `GET /auth/me`
Auth required. Returns the current `user` object.

### `POST /auth/logout`
Auth required. `{ "refresh_token": "..." }` revokes the refresh token.

---

## Public website (no auth; tenant via `X-Tenant`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/public/listings` | Public, available listings |
| GET | `/public/listings/{id}` | One listing |
| GET | `/public/theme` | Tenant branding (company, colours, mode) |
| POST | `/public/applications` | Submit an application → enqueues a screening job |

`POST /public/applications`:
```json
// request
{ "listing_id": "...", "applicant_name": "Taylor Brooks", "email": "t@e.com",
  "phone": "(503) 555-0188", "annual_income_cents": 7800000, "credit_score": 724,
  "move_in": "Aug 1" }
// response
{ "application_id": "...", "status": "Screening",
  "screening_job_id": "...", "message": "Application received — screening in progress" }
```

---

## Landlord / PM console (JWT; tenant-scoped; RBAC)

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/portfolio/summary` | `property:read` | KPI summary (revenue, units, occupancy) |
| GET | `/portfolio/llcs` | `property:read` | Properties grouped by LLC |
| GET | `/properties` | `property:read` | Portfolio list |
| POST | `/properties` | `property:write` | Add a property |
| GET | `/properties/{id}` | `property:read` | **Full profile w/ computed economics** |
| PATCH | `/properties/{id}` | `property:write` | Update property |
| GET | `/llcs` | `property:read` | Holding entities |
| POST | `/llcs` | `tenant:manage` | Create LLC |
| GET | `/applications` | `application:read` | Applications |
| PATCH | `/applications/{id}` | `application:write` | Advance status (Approve → auto-email job) |
| GET | `/theme` | — | Tenant theme |
| PUT | `/theme` | `theme:write` | Update branding + legal templates |
| GET | `/api-tokens` | `apitoken:manage` | List vendor tokens |
| POST | `/api-tokens` | `apitoken:manage` | Mint a token (secret returned once) |
| DELETE | `/api-tokens/{id}` | `apitoken:manage` | Revoke a token |

Property profile economics (mirrors the prototype): `maintenance ≈ 9%`, `taxes &
insurance ≈ 12%`, `management fee = 8%` of rent; `net = rent − maintenance −
taxes − management`.

---

## Modules (JWT; tenant-scoped; `tenant:manage`)

Manage which pluggable modules are enabled for the tenant. See `docs/MODULES.md`.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/modules` | Every module with its resolved `enabled` state for the active tenant |
| PATCH | `/modules/{key}` | `{ "enabled": bool }` — toggle a module (404 for unknown keys) |
| GET | `/modules/flips/pipeline` | Flip deal board (preview; requires `property:read` **and** the `flips` module enabled, else `403`) |

`GET /modules` item shape:
```json
{ "key": "flips", "name": "Acquisitions & Flips", "description": "…",
  "permissions": ["property:read","property:write"],
  "enabled": false, "default_enabled": false, "preview": true }
```

---

## Platform admin (JWT; **staff only**, `platform:admin`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/platform/tenants` | Every client company + property count / managed rent |
| GET | `/platform/metrics` | Tenant/property/revenue totals |

Client users receive `403` here.

---

## Vendor API (API key; scoped)

| Method | Path | Required scope |
|--------|------|----------------|
| GET | `/api/v1/listings` | `listing:read` |
| GET | `/api/v1/properties` | `property:read` |

Example:
```bash
curl http://localhost:8000/api/v1/listings \
  -H "Authorization: Bearer acre_live_xxxxxxxx"
```
A token missing the required scope receives `403`; revoked/expired tokens `401`.

---

## Permissions

`property:read` · `property:write` · `listing:read` · `listing:write` ·
`application:read` · `application:write` · `tenant:manage` · `billing:read` ·
`theme:write` · `apitoken:manage` · `platform:admin` (super-permission).

### System roles

| Role | Permissions |
|------|-------------|
| `platform_admin` | all (Acre HQ staff) |
| `pm_admin` | everything within a tenant |
| `landlord` | property/listing/application read+write |
| `maintenance` | `property:read` |
| `tenant` | `listing:read` |
