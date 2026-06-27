# Investor Onboarding, Financing & Workflows

Acre Nexus models the full lifecycle a property investor cares about: bringing a
house onto the platform with all its details, the financing behind it, the
people/organisations involved, and the strategy-specific workflow it moves
through.

---

## Onboarding

`POST /properties/onboard` (permission `property:write`) is a single,
transactional intake that:

1. creates the **property** with its investor classification (type + strategy),
2. attaches any **mortgages/loans**, creating lender **entities** on the fly when
   only a name is given,
3. starts the property's **workflow** at the first stage of its strategy (and
   records the initial `workflow_event`), and
4. (optionally) enqueues **enrichment** (geocode + parcel/tax/valuation/schools/
   utilities — see `docs/PROPERTY_DATA.md`).

It is audited as `property.onboard`. The console exposes it as a 3-step wizard at
`/console/properties/onboard` (Property → Financing → Review).

---

## Entities registry (counterparties)

The "who's who" of an investor's business — banks, lenders, insurers, title
companies, contractors, inspectors, appraisers, attorneys, utilities. Delivered
as the **`entities`** module.

- `counterparty` — the org/contact (kind, name, contact, channels, inline notes).
- `counterparty_note` — a timestamped note log (append-only history).

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/entities?kind=` | `entity:read` | List, optionally filtered by kind |
| POST | `/entities` | `entity:manage` | Create an entity |
| GET | `/entities/{id}` | `entity:read` | Entity + its note log |
| PATCH | `/entities/{id}` | `entity:manage` | Edit an entity |
| POST | `/entities/{id}/notes` | `entity:manage` | Add a note |

Mortgage lenders link to a counterparty (`mortgage.lender_id`), so a bank's notes
live in one place. UI: `/console/entities`.

---

## Financing (mortgages)

A property can carry several loans (1st/2nd lien). Each `mortgage` records the
lender (as an entity), kind (`purchase`/`refinance`/`heloc`/`private`/
`hard_money`/`seller_finance`), position, original amount, current balance, rate
(bps), term, monthly payment, escrow, dates, loan number, and status.

| Method | Path | Permission |
|--------|------|-----------|
| GET | `/properties/{id}/mortgages` | `finance:read` |
| POST | `/properties/{id}/mortgages` | `finance:manage` |
| PATCH | `/mortgages/{id}` | `finance:manage` |
| DELETE | `/mortgages/{id}` | `finance:manage` |

### Financing feeds the economics

`GET /properties/{id}` now returns levered figures alongside the NOI economics:

- **Debt service** — sum of active mortgage payments (+ escrow), added as a line
  in the cost breakdown.
- **Cash flow after debt** — net operating income − debt service (a KPI when the
  property is financed).
- **Loan balance** + **equity** — equity = best-known value (latest AVM estimate,
  else purchase price) − outstanding loan balances.

---

## Investment workflows

Each property follows a **strategy**, and each strategy has an ordered set of
**stages** (a code-defined catalog in `api/src/workflow.rs`):

| Strategy | Stages |
|----------|--------|
| `rental` | acquisition → rehab/turn → stabilize → leased → managing |
| `flip` | sourcing → under contract → rehab → listed → sold |
| `brrrr` | acquisition → rehab → rent → refinance → repeat |
| `hold` | acquisition → managing |
| `wholesale` | sourcing → under contract → assigned → closed |

A property tracks its `workflow_stage`; every transition is written to
`workflow_event` (from → to, note, actor, time) for a full history.

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/properties/{id}/workflow` | `property:read` | Strategy, current stage, the stage template, and history |
| POST | `/properties/{id}/workflow/advance` | `property:write` | Move to a stage (validated against the strategy); audited `workflow.advance` |

The property profile renders a clickable stage tracker and the recent history.

---

## Schema summary (migration `m20240101_000008_investing`)

- `counterparty`, `counterparty_note` — entities registry + notes.
- `mortgage` — property financing.
- `workflow_event` — workflow transition history.
- `property` gains `property_type`, `strategy`, `workflow_stage`,
  `purchase_price_cents`, `acquired_on`.

New permissions: `entity:read`, `entity:manage`, `finance:read`,
`finance:manage` (granted to workspace owners and property managers; read-level to
landlords). Modules: **`properties`** gains onboarding/financing/workflow routes;
**`entities`** is new (both on by default).
