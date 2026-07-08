# Acquisitions & Underwriting

The buy-side of the platform: an **acquisition deal pipeline** and an
**investor-grade underwriting** calculator, delivered as the **`flips`** module
(`docs/MODULES.md`). This is the investor-depth differentiator incumbents treat
as a second-class citizen — Acre models the whole lifecycle a deal moves through
*before* it becomes a managed property, and underwrites it on real numbers.

Roadmap Phase 7 · issues #41 (underwriting calculators) and #42 (deal pipeline +
data room).

---

## The pipeline

A **`deal`** is a prospective property moving through the acquisition stages
(a code-defined catalog in `api/src/deals.rs`, mirroring the investment
[`workflow`](INVESTING.md#investment-workflows) catalog so the frontend can
render it generically):

```
prospecting → offer → under_contract → closing → owned
                                                   └ dead  (off-ramp: passed / lost)
```

Every deal has an **exit strategy** (`flip` / `brrrr` / `rental` / `hold` /
`wholesale`) reused from the workflow catalog — the strategy the property will
follow once it is owned. Each transition is written to `deal_event` (from → to,
note, actor, time) for a full timeline.

When a deal reaches the end, **one click converts it into a fully-onboarded
`property`**: a new property row is created (name / address / strategy / price /
rent copied over), its investment workflow starts at the strategy's first stage,
the deal is marked `owned` and linked to the property (`converted_property_id`),
and the whole thing is audited. Conversion is idempotent — a deal that already
converted is rejected.

---

## Underwriting

`api/src/underwriting.rs` is a pure, deterministic finance engine (no I/O, fully
unit-tested). Given a deal's stored **assumptions** it computes the metrics every
investor underwrites on:

| Metric | Definition |
|--------|-----------|
| **Cap rate** | NOI ÷ all-in cost basis (purchase + rehab + closing). |
| **Cash-on-cash** | Annual pre-tax cash flow ÷ cash invested (down + closing + rehab). |
| **DSCR** | NOI ÷ annual debt service — the lender's coverage ratio. |
| **IRR** | Levered internal rate of return over the hold, solved by bisection on the yearly cash-flow stream (operating cash flow each year + net sale proceeds in the exit year). |

Supporting figures come back too: monthly/annual debt service (fully-amortising
payment), effective gross income after vacancy, NOI, projected **exit value**
(income approach off the exit cap rate, or appreciation when no cap is given),
loan payoff at exit, net sale proceeds, and total profit over the hold.

### Sensitivity

Rather than a single point estimate, the engine returns a **rent-growth
sensitivity band** — IRR recomputed at −2, −1, 0, +1, +2 percentage points around
the base rent-growth assumption — so the operator sees the range, not a false
precision.

### Assumptions

All stored on the deal (nullable; the engine applies sensible defaults for any
knob left blank — 5% vacancy, 20% down, 7% APR, 30-yr term, 3% growth/appreciation,
7% cost of sale, 5-yr hold): purchase price (offer → asking fallback), rehab,
ARV, closing costs, monthly rent + expenses, vacancy, down payment, interest
rate, loan term, rent growth, appreciation, exit cap rate, selling costs, hold
years. Money is integer cents; rates are basis points.

### What-if

The console recomputes **live** against a stateless endpoint — the operator
drags the assumption knobs, the metrics + sensitivity update, and nothing is
persisted until they **Save**. Saving stores the assumptions on the deal so its
underwriting is reproducible.

---

## Data room

Due-diligence files (offers, LOIs, inspection reports, title commitments) ride
the existing polymorphic [`document`](INTEGRATIONS.md) service with
`owner_type = "deal"` — the same upload / version / signed-URL-download / retention
machinery as every other record. Each deal also carries a JSON **due-diligence
checklist** (`[{ key, label, done, note }]`) editable from the console.

---

## API

All under the `flips` module (JWT; tenant-scoped; self-gated on the module being
enabled), behind the `deal:read` / `deal:write` permissions:

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/modules/flips/pipeline` | `deal:read` | The board: stage taxonomy + every deal with computed underwriting |
| GET | `/modules/flips/deals?stage&strategy` | `deal:read` | List deals (optionally filtered), each with underwriting |
| POST | `/modules/flips/deals` | `deal:write` | Create a deal (starts at `prospecting`); audited `deal.create` |
| GET | `/modules/flips/deals/{id}` | `deal:read` | A deal with underwriting and its event timeline |
| PATCH | `/modules/flips/deals/{id}` | `deal:write` | Edit fields + underwriting assumptions; audited `deal.update` |
| POST | `/modules/flips/deals/{id}/stage` | `deal:write` | Move to a stage (validated); audited `deal.stage_advance` |
| POST | `/modules/flips/deals/{id}/underwrite` | `deal:read` | Stateless "what-if": compute with ad-hoc assumption overrides (persists nothing) |
| PATCH | `/modules/flips/deals/{id}/checklist` | `deal:write` | Replace the due-diligence checklist |
| POST | `/modules/flips/deals/{id}/convert` | `deal:write` + `property:write` | Convert into an owned property; audited `deal.convert` |

Deal documents use the shared document endpoints (`GET`/`POST /documents` with
`owner_type=deal`).

---

## Schema (migration `m20240101_000035_deals`)

- `deal` — the prospective property + offer terms + underwriting assumptions +
  JSON checklist + `converted_property_id`.
- `deal_event` — the deal's timeline (created / stage_change / note / converted).

Both are tenant-owned with enforced Postgres RLS, like every other scoped table.

---

## Frontend

- `/console/flips` — the **acquisition board**: kanban columns over the pipeline
  stages, each deal card showing its price and headline cap rate / IRR, plus a
  create-deal form.
- `/console/flips/[id]` — the **deal workspace**: a stage tracker, the interactive
  underwriting calculator (assumption inputs → cap rate / cash-on-cash / IRR /
  DSCR + returns breakdown + sensitivity band), a due-diligence checklist, the
  data room (reusing `DocumentsCard`), the event timeline, and the
  **Convert to property** action.

The `flips` module is now GA (on by default); Northwind's demo workspace seeds
three deals so the board and underwriting render out of the box.
