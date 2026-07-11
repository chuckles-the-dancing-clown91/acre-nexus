# Deployment & Infrastructure

How Acre Nexus is containerized and deployed (issue #66). It ties together the
prod-safety guards (#23–#25) and the RLS second wall (#27): in production the app
**fails closed** on missing secrets, does **not** migrate on boot, and connects
as a **non-owner** database role so row-level security actually enforces.

## Images

Two images, built by `.github/workflows/deploy.yml` and published to GHCR on
`main`:

- **backend** (`backend/Dockerfile`) — a multi-stage Rust build shipping the
  `api` server and the `migration` binary on a slim, non-root Debian runtime.
- **frontend** (`frontend/Dockerfile`) — the Next.js `standalone` output on a
  slim, non-root Node runtime.

Local / staging stack: `docker compose up` (see `docker-compose.yml`) runs
Postgres + backend + frontend with dev-shaped config.

## Production configuration (env)

`APP_ENV=production` turns on fail-closed startup. These MUST be set (the server
refuses to boot otherwise — see [`backend/README.md`](../backend/README.md#production-safety-app_envproduction)):

| Var | Purpose |
|---|---|
| `APP_ENV=production` | Enables fail-closed key handling + AUTO_MIGRATE-off |
| `JWT_SECRET` | ≥32 chars, not the dev default (`openssl rand -hex 32`) |
| `PII_ENC_KEY` | 64 hex chars (`openssl rand -hex 32`) |
| `SECRETS_ENC_KEY` | 64 hex chars, independent of `PII_ENC_KEY` |
| `DATABASE_URL` | Points at the **`acre_app`** role (below), not the owner |

Leave `AUTO_MIGRATE` **unset** in prod (it defaults off): the app must not
migrate or seed on boot. Manage all of the above as platform secrets, never in
the image or compose file.

## Database: migrations vs. the app role (RLS)

Two distinct roles, by design (this is what makes RLS a real second wall):

1. **Owner / migration role** — owns the schema and runs migrations as an
   explicit deploy step:

   ```bash
   DATABASE_URL="$OWNER_DATABASE_URL" migration up      # (or cargo run -p migration -- up)
   ```

2. **`acre_app`** — the API's runtime role: **NOSUPERUSER, NOBYPASSRLS**, no DDL.
   Because it is neither a superuser nor the table owner, `FORCE ROW LEVEL
   SECURITY` + the per-table isolation policies bite, and the per-request
   `SET LOCAL app.tenant_id` (in `api::db::RequestDb`) scopes every query.

Provision `acre_app` **once per database**, as the owner/migration role, with
[`backend/deploy/roles.sql`](../backend/deploy/roles.sql):

```bash
psql "$OWNER_DATABASE_URL" -v dbname=acre -v app_password="$ACRE_APP_PASSWORD" \
  -f backend/deploy/roles.sql
```

It creates the role, grants CRUD (no DDL) on current + future tables via
`ALTER DEFAULT PRIVILEGES`, and asserts the role can't bypass RLS. Then point the
API's `DATABASE_URL` at `acre_app`.

### Deploy order

1. `migration up` as the owner role (applies schema, RLS policies).
2. `roles.sql` as the owner role (once, or after adding a new owner).
3. Roll out the backend image with `DATABASE_URL` → `acre_app`.
4. Roll out the frontend image.

## Verifying RLS bites in your deployment

Connected as `acre_app`, a tenant-scoped transaction sees only that tenant's
rows, and the platform (unset) context sees all — the same check the integration
suite runs (`rls_bites_for_a_non_superuser_role`):

```sql
BEGIN;
SELECT set_config('app.tenant_id', '<a-tenant-uuid>', true);
SELECT count(*) FROM property;   -- only that tenant's rows
ROLLBACK;
```

If this returns other tenants' rows, the app is connected as a superuser/owner —
fix the role before going live.

## Observability

The backend exposes Prometheus metrics at `/metrics` and correlates errors to
`X-Request-Id`; see [`OBSERVABILITY.md`](OBSERVABILITY.md).
