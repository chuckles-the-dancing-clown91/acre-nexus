# Acre backend (Rust)

Rocket + SeaORM (Postgres) + Tokio. A Cargo workspace:

- `crates/entity` — SeaORM models (documented; money as integer cents)
- `crates/migration` — schema + RLS migrations
- `crates/api` — the Rocket application (auth, RBAC, tenancy, tokens, scheduler, routes)

## Run

```bash
cp .env.example .env          # set DATABASE_URL / JWT_SECRET
createdb acre                 # ensure the database exists
cargo run -p api              # migrates + seeds + serves on :8000
```

With `AUTO_MIGRATE=1` (default) the server runs migrations and seeds demo data on
boot. Demo logins (password `password`): `avery@acrehq.com` (staff),
`jordan@northwind.com`, `priya@cascade.com`.

## Migrations

```bash
cargo run -p migration -- up       # apply
cargo run -p migration -- down     # rollback last
cargo run -p migration -- status   # list
```

## Tests

```bash
cargo test
```

See `../docs/API.md` for the endpoint reference and `../ARCHITECTURE.md` for design.
