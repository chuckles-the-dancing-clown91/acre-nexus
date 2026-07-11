# ADR 0001 — `rocket_okapi` maintenance decision

- **Status:** Accepted
- **Track:** T0 hardening (#22), resolves #29
- **Date:** 2026-07

## Context

The API's OpenAPI document (served at `/openapi.json`, explored via Swagger UI +
RapiDoc) is generated at compile time by [`rocket_okapi`] `0.9`, which sits on
top of `schemars` `0.8` for JSON-Schema derivation. The coupling is pervasive:

- **259** route files carry the `#[rocket_okapi::openapi]` attribute,
- **23** module `api()` functions build their route+spec pair with the
  `openapi_get_routes_spec!` macro,
- **72** files derive `schemars::JsonSchema` on their request/response DTOs.

`rocket_okapi` is **thinly maintained** relative to `rocket` itself — a small
maintainer team and a slower release cadence — and it is pinned to a specific
`rocket` minor. That is an acceptable *today* but an unquantified *tomorrow*
risk: if a future `rocket` major ships before `rocket_okapi` follows (or it goes
unmaintained), our OpenAPI generation blocks the upgrade. #29 asks for a
**decision**, not an assumption.

## Options considered

1. **Pin the current version and isolate it.** Keep `rocket_okapi 0.9`
   (already pinned via `Cargo.lock`), treat the two macro touchpoints as the
   entire coupling surface, and define explicit triggers + an exit path.
2. **Migrate to [`utoipa`].** `utoipa` is the actively-maintained OpenAPI
   generator in the Rust ecosystem — but it has **no first-class Rocket route
   integration** (its adapters target axum/actix/warp). Adopting it on Rocket
   means hand-wiring every path/operation into a `utoipa::OpenApi` derive
   separately from the `#[get]`/`#[post]` handlers — i.e. re-annotating all 259
   routes and re-deriving 72 DTOs (`ToSchema` instead of `JsonSchema`), with the
   route↔spec correspondence no longer enforced by one macro. A spike confirmed
   the blocker is architectural (no Rocket adapter), so the spike's cost is the
   migration's cost — there is no cheap partial adoption.
3. **Hand-author a static `openapi.json`.** Drop all per-route attributes and
   maintain the spec by hand. Lowest dependency risk, highest drift risk (the
   spec silently rots out of sync with the handlers), and a large one-time
   authoring cost for the current surface.

## Decision

**Adopt Option 1: pin `rocket_okapi 0.9` and keep it isolated, with documented
triggers and an exit path.**

Rationale: the dependency works today, is pinned reproducibly, and the migration
cost of Options 2/3 is high and buys nothing until `rocket_okapi` actually falls
behind. Paying that cost now would be speculative. The risk is real but
*bounded and contained*, not ambient — which is exactly what #29 asks for.

### Isolation (so a future migration stays mechanical)

The **only** coupling surface to `rocket_okapi` is:

- the `#[rocket_okapi::openapi(tag = "…")]` attribute on each handler,
- the `openapi_get_routes_spec![…]` macro in each module's `api()`,
- the `merge_specs` / `get_openapi_route` / Swagger / RapiDoc wiring in
  `main.rs::build_rocket`,
- the `schemars::JsonSchema` derives on DTOs.

Handlers are otherwise ordinary Rocket routes. Nothing in the business logic,
guards, or persistence layer depends on `rocket_okapi`. A migration is therefore
a **mechanical, greppable** transformation confined to those four points, not a
cross-cutting rewrite.

### Exit path (if a trigger fires)

Fastest escape hatch: replace `get_openapi_route` with a route that serves a
**hand-authored (or last-known-good generated) `openapi.json`** and delete the
per-route attributes + the macro, keeping the handlers as plain
`routes![…]`. This decouples the doc from the framework version immediately, at
the cost of manual spec upkeep — an acceptable stopgap while a full `utoipa`
migration is scheduled.

### Review triggers

Revisit this ADR when **any** of these becomes true:

- We upgrade `rocket` to `0.6`+ (check `rocket_okapi` compatibility first — it
  may gate the upgrade).
- `rocket_okapi` publishes no release for **12 months**, or is formally
  archived/deprecated.
- A security advisory is filed against `rocket_okapi` or its transitive
  `schemars`/`okapi` deps (the dependency-audit gate in CI, #65, will surface
  this automatically).

## Consequences

- No code change today beyond this record; `Cargo.lock` already pins `0.9.0`.
- The dependency-currency automation (#65) watches `rocket_okapi` like any other
  dependency, so "unmaintained" becomes a visible signal rather than a surprise.
- The coupling surface is documented, so whoever executes a future migration
  knows its exact extent up front.

[`rocket_okapi`]: https://crates.io/crates/rocket_okapi
[`utoipa`]: https://crates.io/crates/utoipa
