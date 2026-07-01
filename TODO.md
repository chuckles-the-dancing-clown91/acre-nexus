# TODO — Next Steps

Findings from a full review of the docs (root + `docs/`) against the actual
backend (`backend/crates/api`) and frontend (`frontend/src`) code, plus a
build/lint/test/clippy health check of both stacks. For the strategic,
multi-phase feature roadmap see **`docs/ROADMAP.md`** — this file is the
tactical punch list: doc corrections and engineering debt found during the
review.

## Health check summary

- **Backend**: `cargo check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, and `cargo test --workspace` all pass clean. No `TODO`/`FIXME`/
  `todo!()`/`unimplemented!()`/stray `panic!()` in non-test code.
- **Frontend**: `npm run lint`, `typecheck`, `test`, `format:check`, and
  `build` all pass clean. No `TODO`/`console.log`/`any`-typed leftovers.
- Both stacks are genuinely healthy day-to-day — the gaps below are about
  **documentation accuracy** and **test coverage depth**, not broken code.

## 1. Doc corrections (quick, high value)

- [ ] **`docs/MODULES.md`**: add the two modules missing from the table —
  `domains` (Domains & Routing, default-on) and `lease_builder` (Lease
  Builder & Tenancy, default-on). Both are registered in
  `backend/crates/api/src/modules/mod.rs` but absent from the doc's table.
- [ ] **`docs/API.md`**: document the endpoints that exist in code but aren't
  mentioned anywhere in the doc: `GET /auth/workspaces` + `POST
  /auth/switch`, `GET /workflows/catalog`, the whole `domains` module routes,
  `vehicles` routes, `platform/impersonations` + `platform/provision`,
  `portfolios` (distinct from `/portfolio/summary`), `applications/workflow`
  + `applications/reuse` + `applications/convert-to-lease`, `cap-table` and
  `bank-accounts` under `/entities/{id}/...`, and `tenant-history` routes.
  Also note the undocumented extra fields on the `POST /auth/login` response
  (`active_tenant_id`, `memberships[]`, `workspaces[]`).
- [ ] **`docs/FEATURES.md`**: several rows understate what's shipped —
  - "Lease generation from templates + e-sign" (row ~58) and "Document
    storage + e-sign" (row ~141) read as if nothing exists; in fact a
    lease-specific e-sign flow already ships (`lease_document` entity +
    `POST /leases/<id>/document/{generate,sign}` with a typed signature +
    SHA-256 hash). Reword to make clear it's **lease-only**, not a general
    document service (the general service is still genuinely unbuilt —
    that part of the doc is correct).
  - "Rent ledger (charges + payments)" (row ~20) says only `lease_payment`
    exists; a full `lease_charge` entity (fees/discounts/rebates/amenities,
    negative-cents credits) already exists too. Update the note.
  - "Audit trail" (row ~106, marked ✅ "every request + change") — every
    *request* is genuinely covered by the fairing, but only ~40% of route
    handlers call `audit::record(...)` for domain events, and those only log
    final state (no before/after diff). Consider a note or a follow-on task
    (see §3) rather than leaving the ✅ unqualified.
- [ ] **`IMPLEMENTATION.md`**: fix the internal contradiction — the "What's
  implemented" section says maintenance work orders + a maintenance board
  ship in the console, but the "What's intentionally deferred" section
  further down still lists "Maintenance work-orders/dispatch" as
  designed-only. Maintenance is shipped; remove it from the deferred list.
- [ ] **`frontend/README.md`**: the "Structure" section lists only 4 console
  subdirectories; there are 18 (`entities`, `leases`, `fees`, `members`,
  `workflows`, `maintenance`, `domains`, `branding`, `onboarding`,
  `settings`, `tenant-history`, etc.). Also re-verify the "Conversion status
  (TODO)" list of pages still on `useEffect`/`useState` — confirm which of
  those have already been migrated before trusting it as current.

## 2. Test coverage gaps (real debt, not a quick fix)

- [ ] **Backend**: all 17 existing tests are pure-logic unit tests (workflow
  transitions, scope-covers logic, dto formatting, etc.) — zero HTTP/route,
  DB-integration, auth, or tenancy-isolation tests. Given RBAC + multi-tenant
  isolation are the platform's core security guarantees, prioritize
  integration tests (spin up a test Postgres) covering: login/refresh/logout,
  permission enforcement on a representative route per module, and
  cross-tenant data isolation (a request for tenant A can never see tenant
  B's rows even with a valid token).
- [ ] **Frontend**: only 1 of 29 `console/**/page.tsx` files has a test
  (`platform/roles/page.tsx`), and `src/lib/api.ts`, `queries.ts`,
  `theme.tsx`, `store.ts` have zero coverage. Prioritize `api.ts` (the
  fetch/auth-header wrapper every request goes through) and a couple of
  high-traffic pages (properties list, applications).
- [ ] Consider wiring the existing Playwright e2e spec (`e2e/home.spec.ts`)
  into CI behind a docker-composed backend + Postgres, since it's currently
  excluded from `.github/workflows/ci.yml` and only runs manually.

## 3. Code quality follow-ups (minor)

- [ ] Backend has 23 `#[allow(...)]` suppressions rather than fixes: 9×
  `#[allow(clippy::too_many_arguments)]` in `backend/crates/api/src/seed.rs`
  (consider grouping args into small structs) and 13×
  `#[allow(dead_code)]`/`#[allow(unused_imports)]` scattered across
  `modules/mod.rs`, `tenancy/*.rs`, `error.rs`, `rbac/*.rs`, `accounting.rs`,
  `settings.rs`, `tokens/principal.rs` — worth a pass to confirm each is
  still needed vs. genuinely dead code that should be deleted.
- [ ] `routes/iam/update_role.rs:56` and `routes/iam/put_profile.rs:46`
  `.unwrap()` a re-fetch of a row just upserted in the same handler — low
  real-world risk, but swapping to `.ok_or(ApiError::Internal)` would avoid
  a panic on a genuine race/delete instead of a clean 500.
- [ ] Frontend lint warnings (non-blocking, `-D warnings` not enforced here):
  unused `eslint-disable` in `src/app/console/page.tsx:33`, unused
  `Membership` import in `src/app/console/platform/users/[id]/page.tsx:17`.

## 4. Frontend migration debt (already tracked, re-listing here)

`frontend/README.md` already flags these pages as still using
`useEffect`/`useState` instead of the established TanStack Query + RHF
pattern — re-verify current status and finish the conversion:
`console/page.tsx`, `console/applications/page.tsx`, `console/llcs/page.tsx`,
`console/modules/page.tsx`, `console/platform/page.tsx`,
`console/properties/[id]/page.tsx`, `console/flips/page.tsx`,
`listings/[id]/page.tsx` (convert to RHF using the existing
`applicationSchema`), `console/login/page.tsx`.

## 5. Feature work

See `docs/ROADMAP.md` — Phase 1 (secrets/KMS, object storage + document
service, webhook framework, notifications) is the next unlock and is
already well-scoped there; no need to duplicate it here.
