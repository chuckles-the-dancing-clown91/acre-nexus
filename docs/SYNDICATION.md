# Investor Syndication

The `syndication` module (issue #13, *Beyond-GA vertical expansions*) turns the
existing cap table into a GP/LP fund vehicle: **capital commitments**, **capital
calls**, and cash **distributions** run through a three-tier **waterfall**. It
builds directly on the entity registry (`owner` / `llc`) and, like every module,
is a migration + entities + per-handler routes gated per-tenant and audited.

Gated by `investor:read` / `investor:manage` and the per-tenant `syndication`
module toggle (on by default). Money is integer cents throughout.

## Model

- **`investor_commitment`** — an owner's committed capital in a legal entity,
  with running `contributed_cents` (funded) and `returned_cents` (returned)
  balances. `role` is `investor` (LP), `manager` (GP — earns carry), or `member`.
- **`capital_call`** / **`capital_call_line`** — a call for capital, split
  pro-rata by committed capital into one line per commitment. Funding the call
  credits each investor's contributed capital.
- **`distribution`** / **`distribution_line`** — a cash distribution, run through
  the waterfall and broken out per investor by tier.

## The waterfall

`api::syndication::run_waterfall` is pure, deterministic integer-cent math (unit
tested like `underwriting`). A distribution flows through three tiers, each fully
paid before the next:

1. **Return of capital** — pay down each investor's unreturned contributed
   capital, pro-rata by that balance.
2. **Preferred return** — a hurdle paid pro-rata to the *preferred owed*, a
   simple one-period rate (`pref_rate_bps`) on contributed capital. *(Deliberate
   simplification — a real preferred accrues/compounds over time; documented as
   such.)*
3. **Profit split (carried interest)** — of the remainder, the GP takes
   `carry_bps` as carry; the rest is split among all investors pro-rata by
   contributed capital. With no GP on the cap table, no carry is taken.

Every tier splits an exact integer-cent pool by the largest-remainder method, so
**the per-investor allocations always sum to the distributed amount** — no cent
is created or lost. Posting a distribution advances each commitment's
`returned_cents` by its return-of-capital tier.

## API

All routes hang off a legal entity (`entity_id` = `llc.id`):

| Method & path | Permission | Purpose |
|---|---|---|
| `GET  /entities/{id}/commitments` | `investor:read` | The commitment stack + totals |
| `POST /entities/{id}/commitments` | `investor:manage` | Add a commitment (existing or new owner) |
| `POST /entities/{id}/capital-calls` | `investor:manage` | Call capital, split pro-rata |
| `POST /capital-calls/{id}/fund` | `investor:manage` | Mark funded → credit contributed capital |
| `POST /entities/{id}/distributions` | `investor:manage` | Run the waterfall + post lines |
| `GET  /entities/{id}/distributions` | `investor:read` | Distribution history with per-investor lines |

**DoD:** commit capital for an LP and a GP, call + fund capital pro-rata, then
distribute cash and see the waterfall split it into return-of-capital, preferred,
LP profit, and GP carry — summing exactly to the amount distributed.

## Schema

Migration `m20240101_000039_syndication` (`investor_commitment`, `capital_call`,
`capital_call_line`, `distribution`, `distribution_line`), all tenant-owned with
enforced RLS.
