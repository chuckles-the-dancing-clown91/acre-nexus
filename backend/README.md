# Acre backend (Rust)

Rocket + SeaORM (Postgres) + Tokio. A Cargo workspace:

- `crates/user`, `crates/property`, `crates/client` — the three **domain crates**,
  one per database. Each bundles its SeaORM entities (`entity`) and its migrations
  (`migration` + a `Migrator`). `acre_user` also hosts the cross-cutting
  `audit_log` and `background_job` tables.
- `crates/entity` — a thin **facade** re-exporting all three domains under the
  historical `entity::*` / `entity::prelude::*` paths (so handlers don't care
  which crate a model lives in).
- `crates/migration` — a thin **facade** + CLI re-exporting the three migrators.
- `crates/api` — the Rocket application (auth, RBAC, tenancy, tokens, scheduler,
  routes), holding **one connection per database** in `AppState`.

## Databases

The platform uses **three** Postgres databases — `acre_user`, `acre_property`,
`acre_client`. Provision them (install + harden Postgres, create the databases
and least-privilege owner/`_app` roles, write the connection env files):

```bash
cd scripts && ./setup-postgres.sh
```

…or for a quick single-database dev setup, create one database and point
`DATABASE_URL` at it (every per-domain URL falls back to it):

```bash
createdb acre && echo 'DATABASE_URL=postgres://localhost:5432/acre' >> .env
```

## Run

```bash
cp .env.example .env          # set the DB urls / JWT_SECRET
cargo run -p api              # migrates all 3 DBs + seeds + serves on :8000
```

With `AUTO_MIGRATE=1` (default) the server runs migrations and seeds demo data on
boot. Demo logins (password `password`): `avery@acrehq.com` (staff),
`jordan@northwind.com`, `priya@cascade.com`.

## Migrations

Each database has its own migrator; the CLI applies all three at once (using the
`*_DATABASE_OWNER_URL` connections):

```bash
cargo run -p migration -- up       # apply pending to all 3 databases
cargo run -p migration -- down     # rollback last on each
cargo run -p migration -- status   # list per database
```

## Tests

```bash
cargo test
```

See `../docs/API.md` for the endpoint reference and `../ARCHITECTURE.md` for design.
