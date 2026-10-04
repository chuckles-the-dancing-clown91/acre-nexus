#!/usr/bin/env bash
# Restore drill (roadmap area 9): prove the newest backup restores, and time it.
#
#   OWNER_DATABASE_URL    the live database (only used to record the drill's result)
#   SCRATCH_ADMIN_URL     a connection that may CREATE/DROP DATABASE (e.g. postgres://.../postgres)
#   BACKUP_IDENTITY       the age private key file that matches BACKUP_RECIPIENT
#   BACKUP_DIR            where backup.sh writes (default /var/backups/acre)
#   BACKUP_FILE           optional: a specific .dump.age to restore instead of the newest
#
# Restores into a throwaway database (acre_restore_drill), checks the schema
# and row counts the business depends on, records the time it took in
# backup_run, and drops the scratch database. Never touches the live database
# beyond that one insert.
set -euo pipefail

: "${OWNER_DATABASE_URL:?set OWNER_DATABASE_URL}"
: "${SCRATCH_ADMIN_URL:?set SCRATCH_ADMIN_URL}"
: "${BACKUP_IDENTITY:?set BACKUP_IDENTITY to the age private key file}"
DIR="${BACKUP_DIR:-/var/backups/acre}"
FILE="${BACKUP_FILE:-$(ls -1t "$DIR"/acre-*.dump.age 2>/dev/null | head -1)}"
[ -n "$FILE" ] && [ -f "$FILE" ] || { echo "no backup found in $DIR" >&2; exit 1; }
SCRATCH="acre_restore_drill"
STARTED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
T0="$(date +%s)"
SCRATCH_URL="${SCRATCH_ADMIN_URL%/*}/$SCRATCH"

record() { # ok detail
  psql "$OWNER_DATABASE_URL" -q -v ON_ERROR_STOP=1 \
    -v started="$STARTED" -v ok="$1" -v loc="$FILE" -v detail="$2" <<'SQL' || true
INSERT INTO backup_run (kind, started_at, finished_at, ok, location, detail)
VALUES ('restore_drill', :'started', now(), :'ok', :'loc', :'detail');
SQL
}
cleanup() { psql "$SCRATCH_ADMIN_URL" -q -c "DROP DATABASE IF EXISTS $SCRATCH" >/dev/null 2>&1 || true; }
fail() { record false "$1"; cleanup; echo "restore drill failed: $1" >&2; exit 1; }
trap 'fail "unexpected error on line $LINENO"' ERR

cleanup
psql "$SCRATCH_ADMIN_URL" -q -c "CREATE DATABASE $SCRATCH" || fail "couldn't create the scratch database"
age -d -i "$BACKUP_IDENTITY" "$FILE" \
  | pg_restore --no-owner --no-privileges --exit-on-error -d "$SCRATCH_URL" \
  || fail "decrypt or pg_restore failed"

# The restored copy must look like the business: migrations applied and the
# core tables holding rows.
COUNTS="$(psql "$SCRATCH_URL" -At -F' ' -c "
  SELECT (SELECT count(*) FROM seaql_migrations),
         (SELECT count(*) FROM tenant),
         (SELECT count(*) FROM property),
         (SELECT count(*) FROM lease),
         (SELECT count(*) FROM maintenance_ticket)")" || fail "the restored database can't be queried"
read -r MIGRATIONS TENANTS PROPERTIES LEASES TICKETS <<<"$COUNTS"
[ "$MIGRATIONS" -gt 0 ] && [ "$TENANTS" -gt 0 ] || fail "restored, but it has no migrations or tenants ($COUNTS)"

SECS=$(( $(date +%s) - T0 ))
trap - ERR
record true "restored in ${SECS}s: ${MIGRATIONS} migrations, ${TENANTS} workspaces, ${PROPERTIES} properties, ${LEASES} leases, ${TICKETS} work orders"
cleanup
echo "restore drill ok in ${SECS}s from $FILE ($COUNTS)"
