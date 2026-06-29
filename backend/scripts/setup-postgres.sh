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
# 4. Writes the connection strings (mode 0600) using the per-DOMAIN env var names
#    the app actually reads (USER_/PROPERTY_/CLIENT_DATABASE_URL + _OWNER_URL):
#    a per-service file each, plus a combined `secrets/acre.env` it can optionally
#    install straight into backend/.env. Also writes a full Markdown change-report.
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
FRESH_INSTALL=0   # set to 1 only when THIS run installed PostgreSQL (vs reusing one)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TS="$(date +%Y%m%d-%H%M%S)"
REPORT="${REPORT:-$SCRIPT_DIR/postgres-setup-report-$TS.md}"
SECRETS_DIR="${SECRETS_DIR:-$SCRIPT_DIR/secrets}"

# ---- change ledger (rendered into the report at the end) ------------------
declare -a PKGS_INSTALLED=() FILES_CHANGED=() SVCS_ENABLED=()
declare -a ROLES_CREATED=() DBS_CREATED=() ENV_FILES=() NOTES=()
# Accumulated `<DOMAIN>_DATABASE_URL` lines for the combined backend .env.
declare -a ENV_LINES=()

# ---- pretty logging -------------------------------------------------------
if [[ -t 1 ]]; then C_B="\033[1m"; C_G="\033[32m"; C_Y="\033[33m"; C_R="\033[31m"; C_0="\033[0m"; else C_B=""; C_G=""; C_Y=""; C_R=""; C_0=""; fi
log()  { printf "${C_G}==>${C_0} %s\n" "$*"; }
step() { printf "\n${C_B}### %s${C_0}\n" "$*"; }
warn() { printf "${C_Y}warn:${C_0} %s\n" "$*" >&2; }
die()  { printf "${C_R}error:${C_0} %s\n" "$*" >&2; exit 1; }

# On any UNHANDLED failure (set -e), say exactly which command + line broke — so a
# mid-run crash is never silent. Does NOT fire for failures already guarded by
# if / || / && / ! (those are intentional).
on_err() {
  local rc=$? cmd=$BASH_COMMAND line=${BASH_LINENO[0]:-?}
  printf "${C_R}error:${C_0} command failed (exit %s) near line %s:\n  %s\n" "$rc" "$line" "$cmd" >&2
  [[ ${#ENV_FILES[@]} -gt 0 ]] && printf "note: credentials already saved to: %s\n" "${ENV_FILES[*]}" >&2
  exit "$rc"
}
trap on_err ERR

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

# Is a cluster already accepting connections on PG_HOST:PG_PORT? Deliberately
# uses pg_isready (NOT sudo) so the "is it up?" decision never depends on being
# able to sudo to the postgres user — which silently fails when sudo needs a
# password and our probe output is suppressed.
cluster_running() {
  if command_exists pg_isready; then
    pg_isready -q -h "$PG_HOST" -p "$PG_PORT" >/dev/null 2>&1 && return 0
  fi
  (exec 3<>"/dev/tcp/${PG_HOST}/${PG_PORT}") >/dev/null 2>&1 && { exec 3>&- 3<&-; return 0; }
  return 1
}

# Cache sudo credentials ONCE, visibly, up front — so the many later
# output-suppressed `sudo` calls (probes, psql_admin) don't fail silently on a
# host where sudo requires a password.
prime_sudo() {
  [[ -z "$SUDO" ]] && return 0                 # running as root: nothing to do
  sudo -n true 2>/dev/null && return 0          # already passwordless / cached
  [[ -e /dev/tty ]] || die "sudo needs a password but there is no terminal to prompt on (run as root or enable passwordless sudo)"
  log "Caching sudo credentials (you may be prompted once)…"
  sudo -v || die "sudo is required to install/configure PostgreSQL"
}

# Ensure the secrets dir exists and is writable by US. A previous run executed as
# root (e.g. `sudo ./setup-postgres.sh`) leaves it owned by root:root mode 700, so
# a later non-root run can't write into it — reclaim it rather than failing.
ensure_secrets_dir() {
  if [[ -e "$SECRETS_DIR" && ! -w "$SECRETS_DIR" ]]; then
    warn "$SECRETS_DIR isn't writable (likely created by an earlier root run); reclaiming ownership"
    $SUDO chown -R "$(id -un)":"$(id -gn)" "$SECRETS_DIR" 2>/dev/null \
      || die "cannot write to $SECRETS_DIR. Fix with: sudo chown -R $(id -un) '$SECRETS_DIR'"
  fi
  mkdir -p "$SECRETS_DIR"
  chmod 700 "$SECRETS_DIR" 2>/dev/null || true
}

# ---- step 1: is PostgreSQL installed? -------------------------------------
postgres_installed() { command_exists psql || command_exists pg_config || command_exists postgres; }

# ---- step 2a: install -----------------------------------------------------
install_postgres() {
  step "Installing PostgreSQL via $PKG"
  case "$PKG" in
    dnf)
      $SUDO dnf install -y postgresql-server postgresql-contrib
      PKGS_INSTALLED+=("postgresql-server" "postgresql-contrib")
      # Data-dir init is handled by ensure_initialized() right before start, so it
      # also covers an already-installed-but-never-initialised cluster.
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

# Fedora/RHEL ship an UN-initialised data dir — PostgreSQL will not start until
# it exists. (Debian/Ubuntu auto-create a cluster on install; Homebrew inits on
# first run.) Idempotent: only initialises when the data dir has no PG_VERSION.
ensure_initialized() {
  [[ "$PKG" == "dnf" ]] || return 0
  local datadir="${PGDATA:-/var/lib/pgsql/data}"
  # The data dir is mode 700/postgres-owned, so test it through sudo — a plain
  # `[[ -f ]]` as the invoking user false-negatives and would try to re-init a
  # live cluster.
  $SUDO test -f "$datadir/PG_VERSION" && return 0
  step "Initialising the PostgreSQL data directory ($datadir)"
  if command_exists postgresql-setup; then
    $SUDO postgresql-setup --initdb
  elif command_exists initdb; then
    $SUDO install -d -o "$PG_OS_USER" -g "$PG_OS_USER" "$datadir"
    $SUDO -u "$PG_OS_USER" initdb --pgdata="$datadir" --auth-host=scram-sha-256
  else
    die "data dir $datadir is not initialised and neither postgresql-setup nor initdb is on PATH"
  fi
  NOTES+=("Initialised data dir at $datadir")
}

# Dump recent service status + journal to stderr to explain a start failure.
dump_service_logs() {
  $SUDO systemctl status "$SERVICE" --no-pager -l 2>&1 | tail -n 20 >&2 || true
  $SUDO journalctl -u "$SERVICE" --no-pager -n 30 2>&1 | tail -n 30 >&2 || true
}

# ---- step 2b: enable + start the service ----------------------------------
start_service() {
  # If a cluster is already up (common on a dev box), don't touch it.
  if cluster_running; then
    log "PostgreSQL already accepting connections on ${PG_HOST}:${PG_PORT}; not starting."
    return 0
  fi
  step "Enabling + starting the service"
  ensure_initialized
  if [[ "$PKG" == "brew" ]]; then
    brew services start postgresql
  else
    if ! $SUDO systemctl enable --now "$SERVICE"; then
      warn "Could not start ${SERVICE}. Recent status / logs:"
      dump_service_logs
      die "failed to start ${SERVICE} — see the logs above (common causes: uninitialised data dir, a port already in use, or a versioned package whose unit isn't '${SERVICE}')."
    fi
  fi
  SVCS_ENABLED+=("$SERVICE")
  # Wait for the socket to accept connections (sudo-free probe).
  for _ in $(seq 1 30); do cluster_running && break; sleep 1; done
  if ! cluster_running; then
    warn "${SERVICE} started but is not accepting connections yet. Recent logs:"
    dump_service_logs
    die "PostgreSQL did not become ready"
  fi
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
  # Copying from /tmp clobbers the SELinux label on Fedora/RHEL (the data dir is
  # postgresql_db_t); restore it so PostgreSQL can still read the file on reload.
  command_exists restorecon && $SUDO restorecon "$file" 2>/dev/null || true
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

# For an ALREADY-installed cluster: make sure our app/owner roles can connect over
# loopback with scram, and that new passwords are hashed with scram — WITHOUT
# touching the cluster's existing listen_addresses/port (that fuller change is
# only made on a fresh install, by harden_postgres). Idempotent; reload-only.
ensure_auth_config() {
  step "Ensuring loopback scram auth for the app roles"
  local hba
  hba="$(psql_admin -tAc 'SHOW hba_file')" || die "could not locate pg_hba.conf"
  [[ -n "$hba" ]] || die "could not locate pg_hba.conf"

  # First match wins in pg_hba; prepend so these take precedence.
  write_managed_block "$hba" prepend "\
local   all             ${ADMIN_DB_USER}                        peer
host    all             all             127.0.0.1/32            scram-sha-256
host    all             all             ::1/128                 scram-sha-256
local   all             all                                     scram-sha-256"

  # Hash NEW passwords with scram (persisted in postgresql.auto.conf, which loads
  # after — and overrides — the main config). Existing md5 passwords still work.
  psql_admin -tAc "ALTER SYSTEM SET password_encryption = 'scram-sha-256'" >/dev/null
  psql_admin -tAc "SELECT pg_reload_conf()" >/dev/null
  NOTES+=("Ensured pg_hba scram rules on loopback + password_encryption=scram-sha-256 (existing listen_addresses/port left unchanged).")
}

# ---- step 3: superuser password -------------------------------------------
set_superuser_password() {
  # Only (re)set the superuser password on a cluster WE installed. On a
  # pre-existing cluster, changing the postgres password is intrusive and
  # unnecessary — provisioning uses local peer auth as the postgres OS user.
  if [[ "$FRESH_INSTALL" != "1" ]]; then
    log "Existing cluster — leaving the postgres superuser password unchanged."
    return 0
  fi
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
  ensure_secrets_dir
  for svc in "${SERVICES[@]}"; do
    local db="${DB_PREFIX}_${svc}" owner="${DB_PREFIX}_${svc}_owner" app="${DB_PREFIX}_${svc}_app"
    local owner_pw app_pw
    owner_pw="$(prompt_secret "Password for OWNER role '$owner' (DDL/migrations)")"
    app_pw="$(prompt_secret "Password for APP role '$app' (runtime, RLS-bound)")"

    log "roles for '$svc'"
    upsert_role "$owner" "$owner_pw"
    upsert_role "$app"   "$app_pw"

    # Save this service's connection env IMMEDIATELY — the moment the roles (and
    # thus the passwords) are real — so a failure in the DB/grant steps below
    # can't cost you the credentials. The application reads PER-DOMAIN vars
    # (USER_/PROPERTY_/CLIENT_DATABASE_URL + _OWNER_URL — see config.rs), NOT a
    # generic DATABASE_URL.
    local domain="${svc^^}"
    local app_url="postgres://${app}:${app_pw}@${PG_HOST}:${PG_PORT}/${db}"
    local owner_url="postgres://${owner}:${owner_pw}@${PG_HOST}:${PG_PORT}/${db}"
    local envf="$SECRETS_DIR/${svc}.env"
    cat > "$envf" <<ENV
# Acre ${svc} database — generated ${TS} by setup-postgres.sh
${domain}_DATABASE_URL=${app_url}
${domain}_DATABASE_OWNER_URL=${owner_url}
ENV
    chmod 600 "$envf"
    ENV_FILES+=("$envf")
    ENV_LINES+=("# ${svc} database (runtime app role + owner/DDL role)")
    ENV_LINES+=("${domain}_DATABASE_URL=${app_url}")
    ENV_LINES+=("${domain}_DATABASE_OWNER_URL=${owner_url}")
    ENV_LINES+=("")

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
  done
}

# Write the combined backend env (all domains in one file) and optionally install
# it into backend/.env, preserving any non-DB settings already there.
finalize_env() {
  step "Writing combined backend env"
  local combined="$SECRETS_DIR/acre.env"
  {
    echo "# Acre Nexus — database connection env, generated ${TS} by setup-postgres.sh."
    echo "# Copy to backend/.env (or source it) before running the API."
    echo
    printf '%s\n' "${ENV_LINES[@]}"
  } > "$combined"
  chmod 600 "$combined"
  ENV_FILES+=("$combined (combined — the file the app needs; copy to backend/.env)")

  local backend_env="$SCRIPT_DIR/../.env"
  if ! confirm "Install these connection strings into backend/.env now?"; then
    log "Skipped backend/.env — copy $combined there yourself when ready."
    return
  fi

  local tmp; tmp="$(mktemp)"
  local db_re='^[[:space:]]*(USER|PROPERTY|CLIENT)_DATABASE(_OWNER)?_URL=|^[[:space:]]*DATABASE_URL='
  if [[ -f "$backend_env" ]]; then
    # Back up into the gitignored secrets dir (NOT next to .env, which would be
    # a tracked path) so the old credentials never risk landing in git.
    local bak="$SECRETS_DIR/dotenv.acre-bak-$TS"
    cp -p "$backend_env" "$bak"
    FILES_CHANGED+=("$backend_env (backup: $bak)")
    grep -vE "$db_re" "$backend_env" > "$tmp" || true        # keep non-DB settings
  elif [[ -f "$SCRIPT_DIR/../.env.example" ]]; then
    grep -vE "$db_re" "$SCRIPT_DIR/../.env.example" > "$tmp" || true   # seed from example
  fi

  {
    printf '%s\n' "${ENV_LINES[@]}"
    if [[ -s "$tmp" ]]; then echo "# ---- other settings ----"; cat "$tmp"; fi
  } > "$backend_env"
  rm -f "$tmp"
  chmod 600 "$backend_env"
  ENV_FILES+=("$backend_env (DB connection strings installed)")
  NOTES+=("Wrote DB connection strings to backend/.env (existing file backed up; non-DB settings preserved).")
  log "backend/.env written"
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
  prime_sudo          # cache sudo creds up front so suppressed sudo probes don't fail
  ensure_secrets_dir  # (re)claim a writable secrets dir before anything writes to it

  step "Step 1 — checking for an existing PostgreSQL install"
  if postgres_installed; then
    log "PostgreSQL already installed ($(psql --version 2>/dev/null || echo present)); skipping install"
    cluster_running || start_service
  else
    log "PostgreSQL not found"
    confirm "Install + harden PostgreSQL and provision ${#SERVICES[@]} databases (${SERVICES[*]})?" \
      || die "aborted by user"
    FRESH_INSTALL=1
    install_postgres
    start_service
    harden_postgres
  fi

  # Belt-and-braces: confirm the cluster is reachable before provisioning.
  cluster_running || start_service

  set_superuser_password
  provision_services
  finalize_env

  # Make app-role auth work on a PRE-EXISTING cluster — done AFTER the databases +
  # credentials are saved, and non-fatally, so a pg_hba/SELinux hiccup can never
  # cost you the output. (Fresh installs already get this via harden_postgres.)
  if [[ "$FRESH_INSTALL" != "1" ]]; then
    ensure_auth_config || warn "couldn't adjust pg_hba/password_encryption automatically; if the app can't connect over 127.0.0.1, add 'host all all 127.0.0.1/32 scram-sha-256' to pg_hba.conf and reload."
  fi

  write_report

  step "Done"
  log "Provisioned: ${DBS_CREATED[*]:-none}"
  log "App connection env (USER_/PROPERTY_/CLIENT_DATABASE_URL): $SECRETS_DIR/acre.env"
  log "Use it by copying to backend/.env (the script can do this for you above)."
  log "Full change report: $REPORT"
  warn "Secrets and the report live under $SECRETS_DIR / scripts/. Keep them out of git."
}

main "$@"
