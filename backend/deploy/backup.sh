#!/usr/bin/env bash
# Nightly encrypted database backup (roadmap area 9).
#
#   OWNER_DATABASE_URL  the owner/migration role (pg_dump needs to read every table)
#   BACKUP_RECIPIENT    an age public key (age1...); only its private key can read the dump
#   BACKUP_DIR          where dumps are written (default /var/backups/acre)
#   BACKUP_S3_URI       optional: s3://bucket/prefix to copy each dump to (needs the aws CLI)
#   BACKUP_KEEP_DAYS    retention for local and S3 copies (default 30)
#
# Each run writes <dir>/acre-<UTC timestamp>.dump.age (pg_dump custom format,
# encrypted with age) and records a row in backup_run so the console's go-live
# page shows when the last good backup ran. Exits non-zero on any failure,
# after recording it.
set -euo pipefail

: "${OWNER_DATABASE_URL:?set OWNER_DATABASE_URL}"
: "${BACKUP_RECIPIENT:?set BACKUP_RECIPIENT to an age public key}"
DIR="${BACKUP_DIR:-/var/backups/acre}"
KEEP="${BACKUP_KEEP_DAYS:-30}"
mkdir -p "$DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
FILE="$DIR/acre-$STAMP.dump.age"
STARTED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

record() { # ok bytes location detail
  psql "$OWNER_DATABASE_URL" -q -v ON_ERROR_STOP=1 \
    -v started="$STARTED" -v ok="$1" -v bytes="$2" -v loc="$3" -v detail="$4" <<'SQL' || true
INSERT INTO backup_run (kind, started_at, finished_at, ok, bytes, location, detail)
VALUES ('backup', :'started', now(), :'ok', NULLIF(:'bytes', '')::bigint, :'loc', NULLIF(:'detail', ''));
SQL
}

fail() { record false "" "$FILE" "$1"; echo "backup failed: $1" >&2; exit 1; }
trap 'fail "unexpected error on line $LINENO"' ERR

pg_dump --format=custom --no-owner --no-privileges "$OWNER_DATABASE_URL" \
  | age -r "$BACKUP_RECIPIENT" -o "$FILE" || fail "pg_dump or age failed"
BYTES="$(stat -c %s "$FILE")"
[ "$BYTES" -gt 1024 ] || fail "the dump is suspiciously small ($BYTES bytes)"

LOCATION="$FILE"
if [ -n "${BACKUP_S3_URI:-}" ]; then
  aws s3 cp --only-show-errors "$FILE" "${BACKUP_S3_URI%/}/$(basename "$FILE")" \
    || fail "copy to $BACKUP_S3_URI failed"
  LOCATION="${BACKUP_S3_URI%/}/$(basename "$FILE")"
fi

# Retention: local files older than KEEP days. (Use an S3 lifecycle rule
# for the bucket copy; it's cheaper and survives this host.)
find "$DIR" -name 'acre-*.dump.age' -type f -mtime +"$KEEP" -delete

trap - ERR
record true "$BYTES" "$LOCATION" ""
echo "backup ok: $LOCATION ($BYTES bytes)"
