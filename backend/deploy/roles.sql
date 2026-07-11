-- Production database roles for row-level security (issue #66; closes the #27
-- "app must connect as a non-owner role" gap).
--
-- Migrations enable + FORCE row-level security on every tenant-owned table, but
-- a table's OWNER and any SUPERUSER bypass RLS. So in production the API must
-- connect as a dedicated role that is NEITHER the table owner NOR a superuser —
-- then FORCE RLS + the per-table isolation policies actually bite, and
-- `SET LOCAL app.tenant_id` (per request, in api::db::RequestDb) scopes reads.
--
-- Deployment shape:
--   * A migration/owner role (e.g. `acre_migrator` or your managed-Postgres
--     admin) OWNS the schema and RUNS migrations: `cargo run -p migration -- up`.
--   * The API connects as `acre_app` (below) for all request traffic.
--
-- Run this ONCE per database, AS THE MIGRATION/OWNER ROLE (so the
-- ALTER DEFAULT PRIVILEGES attach to the tables that role will create), e.g.:
--   psql "$OWNER_DATABASE_URL" -v dbname=acre -v app_password="$ACRE_APP_PASSWORD" -f roles.sql
-- Then point the API's DATABASE_URL at acre_app.

\set ON_ERROR_STOP on

-- 1. The application login role: not a superuser, does not bypass RLS.
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'acre_app') THEN
    CREATE ROLE acre_app LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
  END IF;
END $$;

-- Set/rotate the password out-of-band (psql var keeps it out of this file).
ALTER ROLE acre_app WITH PASSWORD :'app_password';

-- 2. Connect + schema usage.
GRANT CONNECT ON DATABASE :"dbname" TO acre_app;
GRANT USAGE ON SCHEMA public TO acre_app;

-- 3. Data privileges on existing objects. Deliberately NO DDL (no CREATE) — the
--    app never migrates in production (AUTO_MIGRATE defaults off, issue #23).
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO acre_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO acre_app;

-- 4. Cover objects created by FUTURE migrations too. These apply to tables
--    created by the CURRENT role (the migration/owner role running this file).
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO acre_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT USAGE, SELECT ON SEQUENCES TO acre_app;

-- Sanity: acre_app must NOT be able to bypass RLS.
DO $$
BEGIN
  IF (SELECT rolbypassrls OR rolsuper FROM pg_roles WHERE rolname = 'acre_app') THEN
    RAISE EXCEPTION 'acre_app must be NOSUPERUSER and NOBYPASSRLS for RLS to bite';
  END IF;
END $$;
