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

With `AUTO_MIGRATE=1` (the dev default) the server runs migrations and seeds demo
data on boot. Demo logins (password `password`): `avery@acrehq.com` (staff),
`jordan@northwind.com`, `priya@cascade.com`.

## Migrations

```bash
cargo run -p migration -- up       # apply
cargo run -p migration -- down     # rollback last
cargo run -p migration -- status   # list
```

## Production safety (`APP_ENV=production`)

Setting `APP_ENV=production` (or `prod`) switches the server to **fail-closed**
startup — it refuses to boot on an insecure config rather than degrade silently:

- **`JWT_SECRET`** — required, and rejected if it's the `.env.example` default
  or shorter than 32 chars. Generate one with `openssl rand -hex 32`.
- **`PII_ENC_KEY`** — required (64 hex chars / 32 bytes). No boot without it;
  it is never derived from `JWT_SECRET` in production.
- **`SECRETS_ENC_KEY`** — required (64 hex chars / 32 bytes), same as above.
- **`AUTO_MIGRATE`** — defaults **off** in production, so the app never runs
  migrations or seeds unattended. Demo data (the `password`-login tenants) is
  **never** seeded in production, even against an empty database. Run schema
  changes as an explicit deploy step instead:

  ```bash
  cargo run -p migration -- up
  ```

  (Set `AUTO_MIGRATE=1` explicitly only if you deliberately want boot-time
  migrations in a given prod environment.)

**Key rotation.** `PII_ENC_KEY` and `SECRETS_ENC_KEY` are independent AES-256-GCM
keys (distinct domains, so a leaked provider credential and a leaked SSN have
separate blast radii). Rotating either today is a manual, coordinated step:
decrypt-with-old / re-encrypt-with-new the affected ciphertext columns, then cut
over the env var. Keep the previous key until re-encryption is confirmed, since
existing ciphertext is only decryptable with the key that wrote it.

## Tests

```bash
cargo test
```

Most tests are pure-logic unit tests and need no services. The **integration
tests** (`crates/api/src/itest.rs`) boot the real Rocket app and exercise
auth/RBAC enforcement and cross-tenant isolation (incl. RLS) over HTTP; they run
only when `TEST_DATABASE_URL` points at a disposable Postgres, and skip
otherwise so a plain `cargo test` stays green with no database:

```bash
createdb acre_it
TEST_DATABASE_URL=postgres://localhost:5432/acre_it cargo test -p api itest
```

CI provisions a throwaway Postgres and sets `TEST_DATABASE_URL`, so these run on
every push (see `.github/workflows/ci.yml`).

See `../docs/API.md` for the endpoint reference and `../ARCHITECTURE.md` for design.
