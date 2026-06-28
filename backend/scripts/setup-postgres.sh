#!/usr/bin/env bash
#
# setup-postgres.sh — Acre Nexus PostgreSQL provisioner
# ---------------------------------------------------------------------------
# 1. Detects whether PostgreSQL is already installed.
# 2. If not, installs it and *hardens* the cluster:
#       - scram-sha-256 password auth (never trust/md5)
#       - listens on localhost only (never 0.0.0.0 by default)
#       - connection + DDL logging on
#    then enables the service.
# 3. Prompts for the postgres superuser password, then provisions ONE
#    database + a least-privilege owner/app role pair per Acre service
#    (user / property / client). App roles are NOBYPASSRLS so the existing
#    row-level-security tenancy keeps biting at runtime.
# 4. Writes a per-service .env (mode 0600) with the connection strings and a
#    full Markdown change-report so you can see — and undo — everything that
#    touched the machine.
#
# Idempotent: safe to run repeatedly. Secrets are generated with a CSPRNG and
# never appear in argv (so they can't leak via `ps`); prompts read from the
# terminal directly.
#
# Usage:
#   ./setup-postgres.sh                         # interactive
#   SERVICES="user property client" ./setup-postgres.sh
#   ASSUME_YES=1 ./setup-postgres.sh            # non-interactive (CI): auto-gen all passwords
#
# Override behaviour with env vars (see the configuration block below).
# ---------------------------------------------------------------------------
set -euo pipefail
umask 077   # everything we create (secrets, reports, temp files) is owner-only

# ---- configuration --------------------------------------------------------
DB_PREFIX="${DB_PREFIX:-acre}"
# One database + owner/app role pair per domain service. Override via env.
read -r -a SERVICES <<< "${SERVICES:-user property client}"
PG_HOST="${PG_HOST:-127.0.0.1}"
PG_PORT="${PG_PORT:-5432}"
LISTEN_ADDRESSES="${LISTEN_ADDRESSES:-localhost}"   # do NOT expose publicly by default
ASSUME_YES="${ASSUME_YES:-0}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TS="$(date +%Y%m%d-%H%M%S)"
REPORT="${REPORT:-$SCRIPT_DIR/postgres-setup-report-$TS.md}"
SECRETS_DIR="${SECRETS_DIR:-$SCRIPT_DIR/secrets}"

# ---- change ledger (rendered into the report at the end) ------------------
declare -a PKGS_INSTALLED=() FILES_CHANGED=() SVCS_ENABLED=()
declare -a ROLES_CREATED=() DBS_CREATED=() ENV_FILES=() NOTES=()

# ---- pretty logging -------------------------------------------------------
if [[ -t 1 ]]; then C_B="\033[1m"; C_G="\033[32m"; C_Y="\033[33m"; C_R="\033[31m"; C_0="\033[0m"; else C_B=""; C_G=""; C_Y=""; C_R=""; C_0=""; fi
log()  { printf "${C_G}==>${C_0} %s\n" "$*"; }
step() { printf "\n${C_B}### %s${C_0}\n" "$*"; }
warn() { printf "${C_Y}warn:${C_0} %s\n" "$*" >&2; }
die()  { printf "${C_R}error:${C_0} %s\n" "$*" >&2; exit 1; }

command_exists() { command -v "$1" >/dev/null 2>&1; }

# Generate a strong, URL-safe password (alnum only → no DATABASE_URL escaping).
gen_password() { LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40; }

# Double single-quotes for safe inlining into an SQL string literal.
sql_escape() { local s="$1"; printf '%s' "${s//\'/\'\'}"; }

# Read a secret from the terminal (twice, confirmed). Blank => auto-generate.
# The secret is returned on stdout; all prompts go to /dev/tty so callers can
# safely do  pw="$(prompt_secret ...)".
prompt_secret() {
  local label="$1" p1 p2
  if [[ "$ASSUME_YES" == "1" || ! -e /dev/tty ]]; then gen_password; return; fi
  while true; do
    read -rsp "$label (leave blank to auto-generate): " p1 </dev/tty; echo >/dev/tty
    if [[ -z "$p1" ]]; then gen_password; return; fi
    read -rsp "  confirm: " p2 </dev/tty; echo >/dev/tty
    [[ "$p1" == "$p2" ]] && { printf '%s' "$p1"; return; }
    printf 'passwords did not match, try again\n' >/dev/tty
  done
}

confirm() {
  [[ "$ASSUME_YES" == "1" || ! -e /dev/tty ]] && return 0
  local a; read -rp "$1 [y/N] " a </dev/tty; [[ "$a" =~ ^[Yy]$ ]]
}

# ---- privilege + platform detection ---------------------------------------
if [[ $EUID -eq 0 ]]; then SUDO=""; else SUDO="sudo"; fi
[[ -z "$SUDO" ]] || command_exists sudo || die "need root or sudo to install/configure PostgreSQL"

OS="$(uname -s)"
PKG=""; PG_OS_USER="postgres"; SERVICE="postgresql"; ADMIN_DB_USER="postgres"
detect_platform() {
  if [[ "$OS" == "Darwin" ]]; then
    PKG="brew"; PG_OS_USER="$(id -un)"; ADMIN_DB_USER="$(id -un)"; SERVICE="postgresql"
    return
  fi
  [[ -r /etc/os-release ]] || die "unsupported Linux: no /etc/os-release"
  . /etc/os-release
  case " ${ID:-} ${ID_LIKE:-} " in
    *fedora*|*rhel*|*centos*|*rocky*|*almalinux*) PKG="dnf" ;;
    *debian*|*ubuntu*)                            PKG="apt" ;;
    *) die "unsupported distro '${ID:-?}'. Install PostgreSQL manually, then re-run." ;;
  esac
}

# Run psql as the cluster superuser over the local socket (peer/trust auth).
# SQL is fed on stdin (never argv) so passwords cannot leak via the process list.
psql_admin() { $SUDO -u "$PG_OS_USER" psql -v ON_ERROR_STOP=1 -X "$@"; }

# ---- step 1: is PostgreSQL installed? -------------------------------------
postgres_installed() { command_exists psql || command_exists pg_config || command_exists postgres; }

# ---- step 2a: install -----------------------------------------------------
install_postgres() {
  step "Installing PostgreSQL via $PKG"
  case "$PKG" in
    dnf)
      $SUDO dnf install -y postgresql-server postgresql-contrib
      PKGS_INSTALLED+=("postgresql-server" "postgresql-contrib")
      # Fedora/RHEL ship an un-initialised data dir; create it explicitly.
      if [[ ! -f /var/lib/pgsql/data/PG_VERSION ]]; then
        $SUDO postgresql-setup --initdb
        NOTES+=("Initialised data dir at /var/lib/pgsql/data via postgresql-setup --initdb")
      fi
      ;;
    apt)
      $SUDO apt-get update -y
      $SUDO DEBIAN_FRONTEND=noninteractive apt-get install -y postgresql postgresql-contrib
      PKGS_INSTALLED+=("postgresql" "postgresql-contrib")
      # Debian/Ubuntu auto-create a "main" cluster on install.
      ;;
    brew)
      brew install postgresql
      PKGS_INSTALLED+=("postgresql (brew)")
      ;;
  esac
}

# ---- step 2b: enable + start the service ----------------------------------
start_service() {
  step "Enabling + starting the service"
  if [[ "$PKG" == "brew" ]]; then
    brew services start postgresql
  else
    $SUDO systemctl enable --now "$SERVICE"
  fi
  SVCS_ENABLED+=("$SERVICE")
  # Wait for the socket to accept connections.
  for _ in $(seq 1 30); do psql_admin -tAc 'SELECT 1' >/dev/null 2>&1 && break; sleep 1; done
  psql_admin -tAc 'SELECT 1' >/dev/null 2>&1 || die "PostgreSQL did not become ready"
}

# Replace (or insert) a marker-delimited managed block in a root-owned file,
# preserving the original as <file>.acre-bak-<ts>. position = prepend|append.
write_managed_block() {
  local file="$1" position="$2" content="$3"
  local begin="# >>> acre-nexus managed (do not edit inside) >>>"
  local end="# <<< acre-nexus managed <<<"
  local stripped block out
  stripped="$(mktemp)"; out="$(mktemp)"
  $SUDO awk -v b="$begin" -v e="$end" '
    $0==b {skip=1} skip!=1 {print} $0==e {skip=0}
  ' "$file" >"$stripped"
  block="$begin"$'\n'"$content"$'\n'"$end"
  if [[ "$position" == "prepend" ]]; then
    { printf '%s\n' "$block"; cat "$stripped"; } >"$out"
  else
    { cat "$stripped"; printf '%s\n' "$block"; } >"$out"
  fi
  $SUDO cp -p "$file" "${file}.acre-bak-$TS"
  $SUDO cp "$out" "$file"
  $SUDO chown --reference="${file}.acre-bak-$TS" "$file" 2>/dev/null || true
  $SUDO chmod --reference="${file}.acre-bak-$TS" "$file" 2>/dev/null || true
  rm -f "$stripped" "$out"
  FILES_CHANGED+=("$file (backup: ${file}.acre-bak-$TS)")
}

# ---- step 2c: harden the cluster ------------------------------------------
harden_postgres() {
  step "Hardening configuration (scram-sha-256, localhost-only, logging)"
  local conf hba
  conf="$(psql_admin -tAc 'SHOW config_file')"
  hba="$(psql_admin -tAc 'SHOW hba_file')"
  [[ -n "$conf" && -n "$hba" ]] || die "could not locate postgresql.conf / pg_hba.conf"

  write_managed_block "$conf" append "\
listen_addresses = '${LISTEN_ADDRESSES}'
port = ${PG_PORT}
password_encryption = scram-sha-256
log_connections = on
log_disconnections = on
log_line_prefix = '%m [%p] %q%u@%d '
log_statement = 'ddl'"

  # First match wins in pg_hba; prepend so our rules take precedence:
  #   - postgres admin keeps local socket (peer) so this script can manage it
  #   - everyone else must present a scram password, loopback only
  write_managed_block "$hba" prepend "\
local   all             ${ADMIN_DB_USER}                        peer
host    all             all             127.0.0.1/32            scram-sha-256
host    all             all             ::1/128                 scram-sha-256
local   all             all                                     scram-sha-256"

  if [[ "$PKG" == "brew" ]]; then brew services restart postgresql; else $SUDO systemctl restart "$SERVICE"; fi
  for _ in $(seq 1 30); do psql_admin -tAc 'SELECT 1' >/dev/null 2>&1 && break; sleep 1; done
  NOTES+=("SSL is left at the cluster default — terminate TLS at a proxy or set ssl=on with certs for remote access.")
}

# ---- step 3: superuser password -------------------------------------------
set_superuser_password() {
  step "Setting the postgres superuser password"
  if ! psql_admin -tAc "SELECT 1 FROM pg_roles WHERE rolname='${ADMIN_DB_USER}'" | grep -q 1; then
    warn "superuser role '${ADMIN_DB_USER}' not found; skipping"
    return
  fi
  local pw; pw="$(prompt_secret "Password for DB superuser '${ADMIN_DB_USER}'")"
  printf "ALTER ROLE \"%s\" WITH LOGIN PASSWORD '%s';\n" "$ADMIN_DB_USER" "$(sql_escape "$pw")" | psql_admin
  printf '%s' "$pw" > "$SECRETS_DIR/_superuser.pw"; chmod 600 "$SECRETS_DIR/_superuser.pw"
  ENV_FILES+=("$SECRETS_DIR/_superuser.pw (superuser password, mode 600)")
  NOTES+=("Superuser '${ADMIN_DB_USER}' now has a password; local socket admin still uses peer auth.")
}

# ---- step 4: per-service databases + least-privilege roles ----------------
db_exists()   { psql_admin -tAc "SELECT 1 FROM pg_database WHERE datname='$(sql_escape "$1")'" | grep -q 1; }
role_attrs="NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS"

upsert_role() { # name password
  local role="$1" pw="$2"
  psql_admin <<SQL
DO \$do\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='$(sql_escape "$role")') THEN
    CREATE ROLE "$role" LOGIN PASSWORD '$(sql_escape "$pw")' ${role_attrs};
  ELSE
    ALTER ROLE "$role" WITH LOGIN PASSWORD '$(sql_escape "$pw")' ${role_attrs};
  END IF;
END
\$do\$;
SQL
  ROLES_CREATED+=("$role")
}

provision_services() {
  step "Provisioning ${#SERVICES[@]} database(s): ${SERVICES[*]}"
  mkdir -p "$SECRETS_DIR"
  for svc in "${SERVICES[@]}"; do
    local db="${DB_PREFIX}_${svc}" owner="${DB_PREFIX}_${svc}_owner" app="${DB_PREFIX}_${svc}_app"
    local owner_pw app_pw
    owner_pw="$(prompt_secret "Password for OWNER role '$owner' (DDL/migrations)")"
    app_pw="$(prompt_secret "Password for APP role '$app' (runtime, RLS-bound)")"

    log "roles for '$svc'"
    upsert_role "$owner" "$owner_pw"
    upsert_role "$app"   "$app_pw"

    if db_exists "$db"; then
      log "database '$db' already exists"
    else
      log "creating database '$db' owned by '$owner'"
      printf 'CREATE DATABASE "%s" OWNER "%s";\n' "$db" "$owner" | psql_admin
    fi
    DBS_CREATED+=("$db (owner=$owner, app=$app)")

    # Lock down PUBLIC; grant only what each role needs. DEFAULT PRIVILEGES make
    # tables the owner creates (via migrations) automatically usable by the app.
    psql_admin -d "$db" <<SQL
REVOKE ALL ON DATABASE "$db" FROM PUBLIC;
GRANT CONNECT ON DATABASE "$db" TO "$owner", "$app";
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT ALL  ON SCHEMA public TO "$owner";
GRANT USAGE ON SCHEMA public TO "$app";
ALTER DEFAULT PRIVILEGES FOR ROLE "$owner" IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO "$app";
ALTER DEFAULT PRIVILEGES FOR ROLE "$owner" IN SCHEMA public
  GRANT USAGE, SELECT ON SEQUENCES TO "$app";
SQL

    # Per-service env file (0600). DATABASE_URL = runtime app role (RLS bites);
    # DATABASE_OWNER_URL = owner role for running migrations.
    local envf="$SECRETS_DIR/${svc}.env"
    cat > "$envf" <<ENV
# Acre ${svc} service — generated ${TS} by setup-postgres.sh
# Runtime connection (least privilege, NOBYPASSRLS):
DATABASE_URL=postgres://${app}:${app_pw}@${PG_HOST}:${PG_PORT}/${db}
# Migration/DDL connection (schema owner) — use for: cargo run -p migration:
DATABASE_OWNER_URL=postgres://${owner}:${owner_pw}@${PG_HOST}:${PG_PORT}/${db}
ENV
    chmod 600 "$envf"
    ENV_FILES+=("$envf")
  done
}

# ---- export: write the change report --------------------------------------
join_lines() { local p="$1"; shift; for x in "$@"; do printf '%s%s\n' "$p" "$x"; done; }

write_report() {
  step "Writing change report"
  local pgver inventory
  pgver="$(psql_admin -tAc 'SHOW server_version' 2>/dev/null || echo unknown)"
  inventory="$(psql_admin -c '\l' 2>/dev/null; echo; psql_admin -c '\du' 2>/dev/null || true)"

  {
    echo "# PostgreSQL setup report — ${TS}"
    echo
    echo "- Host: \`$(hostname)\`  ·  OS: \`${OS}\`  ·  Package manager: \`${PKG}\`"
    echo "- PostgreSQL server version: \`${pgver}\`"
    echo "- Listen: \`${LISTEN_ADDRESSES}:${PG_PORT}\`  ·  Auth: \`scram-sha-256\`"
    echo
    echo "## Packages installed";    [[ ${#PKGS_INSTALLED[@]} -gt 0 ]] && join_lines '- ' "${PKGS_INSTALLED[@]}" || echo "_(none — already installed)_"
    echo
    echo "## Services enabled";      [[ ${#SVCS_ENABLED[@]} -gt 0 ]]   && join_lines '- ' "${SVCS_ENABLED[@]}"   || echo "_(none)_"
    echo
    echo "## Config files changed";  [[ ${#FILES_CHANGED[@]} -gt 0 ]]  && join_lines '- ' "${FILES_CHANGED[@]}"  || echo "_(none)_"
    echo
    echo "## Databases created";     [[ ${#DBS_CREATED[@]} -gt 0 ]]    && join_lines '- ' "${DBS_CREATED[@]}"    || echo "_(none)_"
    echo
    echo "## Roles created/updated"; [[ ${#ROLES_CREATED[@]} -gt 0 ]]  && join_lines '- ' "${ROLES_CREATED[@]}"  || echo "_(none)_"
    echo
    echo "## Secret / env files (mode 0600 — NOT committed)"
    join_lines '- ' "${ENV_FILES[@]}"
    echo
    echo "## Notes"; [[ ${#NOTES[@]} -gt 0 ]] && join_lines '- ' "${NOTES[@]}" || echo "_(none)_"
    echo
    echo "## Live cluster inventory"
    echo '```'
    echo "$inventory"
    echo '```'
    echo
    echo "## How to undo"
    echo "- Restore config: copy each \`*.acre-bak-${TS}\` back over its original and restart the service."
    echo "- Drop databases/roles:"
    echo '```sql'
    for d in "${DBS_CREATED[@]}"; do echo "DROP DATABASE IF EXISTS \"${d%% *}\";"; done
    for r in "${ROLES_CREATED[@]}"; do echo "DROP ROLE IF EXISTS \"$r\";"; done
    echo '```'
    echo "- Remove packages: \`${SUDO} ${PKG} remove postgresql*\` (purges nothing under the data dir)."
  } > "$REPORT"
  chmod 600 "$REPORT"
  log "report → $REPORT"
}

# ---- main -----------------------------------------------------------------
main() {
  detect_platform
  mkdir -p "$SECRETS_DIR"

  step "Step 1 — checking for an existing PostgreSQL install"
  if postgres_installed; then
    log "PostgreSQL already installed ($(psql --version 2>/dev/null || echo present)); skipping install"
  else
    log "PostgreSQL not found"
    confirm "Install + harden PostgreSQL and provision ${#SERVICES[@]} databases (${SERVICES[*]})?" \
      || die "aborted by user"
    install_postgres
    start_service
    harden_postgres
  fi

  # If it was already installed we still ensure it is running before provisioning.
  psql_admin -tAc 'SELECT 1' >/dev/null 2>&1 || start_service

  set_superuser_password
  provision_services
  write_report

  step "Done"
  log "Provisioned: ${DBS_CREATED[*]:-none}"
  log "Credentials + connection strings: $SECRETS_DIR/*.env (mode 0600)"
  log "Full change report: $REPORT"
  warn "Secrets and the report live under $SECRETS_DIR / scripts/. Keep them out of git."
}

main "$@"
