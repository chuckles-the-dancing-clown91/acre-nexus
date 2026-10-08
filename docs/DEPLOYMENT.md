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
| `APP_ENV=production` | Enables fail-closed key handling + AUTO_MIGRATE-off. Only an explicit `development`/`dev`/`local`/`test`/`testing`/`ci` value enables the sandbox sign-in provider, so unset is production-safe too |
| `JWT_SECRET` | ≥32 chars, not the dev default (`openssl rand -hex 32`) |
| `PII_ENC_KEY` | 64 hex chars (`openssl rand -hex 32`) |
| `SECRETS_ENC_KEY` | 64 hex chars, independent of `PII_ENC_KEY` |
| `DATABASE_URL` | Points at the **`acre_app`** role (below), not the owner |

Social sign-in (Google / Microsoft / Apple) is offered only for providers named
in `LIVE_PROVIDERS` whose `oauth.<provider>.client_id` / `client_secret` are in
the secrets vault; with none, the login page shows no social buttons (see
[IAM](IAM.md#log-in-with-google--microsoft--apple-oauth-20--oidc)).

Optional, for the Solnyxus product link ([Tenancy](TENANCY.md#provisioning-from-solnyxus-the-product-link)):
`SOLNYXUS_PLATFORM_KEY` (the key Solnyxus presents; unset → its key-guarded
routes answer `503 not_configured`) and `APP_COMMIT` (the deployed commit, shown
by `/.well-known/solnyxus/health` and `version`). `PUBLIC_APP_URL` must be the
public web address — the set-password and sign-in links handed to Solnyxus are
built from it.

The frontend lists the seeded demo accounts on the login page only when built
with `NEXT_PUBLIC_SHOW_DEMO_ACCOUNTS=1`; production builds leave it unset.

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

## Backups and the restore drill

Two scripts in `backend/deploy/`, run by the host's scheduler (cron, a
systemd timer, or the platform's job runner) as the owner role:

```bash
# Nightly, e.g. 03:15
OWNER_DATABASE_URL=... BACKUP_RECIPIENT=age1... \
BACKUP_DIR=/var/backups/acre BACKUP_S3_URI=s3://acre-backups/prod BACKUP_KEEP_DAYS=30 \
  backend/deploy/backup.sh

# Monthly (and after any restore-affecting change)
OWNER_DATABASE_URL=... SCRATCH_ADMIN_URL=postgres://owner@db/postgres \
BACKUP_IDENTITY=/secure/acre-backup.key BACKUP_DIR=/var/backups/acre \
  backend/deploy/restore-drill.sh
```

- **`backup.sh`** runs `pg_dump --format=custom`, encrypts the stream with
  [age](https://age-encryption.org) to `BACKUP_RECIPIENT` (generate the pair
  once with `age-keygen`; keep the private key off the database host), checks
  the file isn't suspiciously small, copies it to `BACKUP_S3_URI` when set,
  deletes local dumps older than `BACKUP_KEEP_DAYS` (30), and records the run
  in `backup_run`. Give the bucket its own 30-day lifecycle rule.
- **`restore-drill.sh`** decrypts the newest dump (or `BACKUP_FILE`), restores
  it into a throwaway `acre_restore_drill` database, checks migrations,
  workspaces, properties, leases and work orders came back, records the time
  it took in `backup_run`, and drops the scratch database. A failed decrypt or
  restore is recorded as a failed drill and exits non-zero.

Both write to `backup_run` (migration 075), so **Admin → Go live** shows the
newest backup and drill and turns red when the last good backup is older than
36 hours or the last drill older than 90 days. On the demo data the drill
restores in about a second (75 migrations, 2 workspaces, 8 properties).

## Go live

**Admin → Go live** (`GET /go-live`, `integrations:manage`) lists every
provider (email, texts, Stripe, Plaid, Checkr, Gusto, Google Maps, FBI crime
data, RentCast) with whether `LIVE_PROVIDERS` switches it on, which vault keys
or environment variables it needs and whether they're present (names only),
its last **real** call and failures this week (simulated calls are audited
with `live: false` and don't count), and for Stripe, Checkr and Twilio when
the last signed webhook arrived. Each is **ready**, **live, not used yet**,
**failing**, **missing setup** or **simulated**. The deploy checks are
production mode, https public addresses, a backup in the last 36 hours and a
restore drill in the last 90 days. The page never calls out; test email and
texts from Notifications.

## Observability

The backend exposes Prometheus metrics at `/metrics` and correlates errors to
`X-Request-Id`; see [`OBSERVABILITY.md`](OBSERVABILITY.md).
