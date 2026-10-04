# Standard PM Reports

The reports every property manager expects (roadmap Phase 8: the standard set
from issue #56, plus owner statements and the 1099 tax export), delivered as the
pluggable **`reports`** module (`docs/MODULES.md`). Each report reads live off
the shipped rentals + general-ledger data — no separate reporting store — and is
viewable in the console and exportable to **CSV or PDF**. Gated by `report:read`.
Money is integer cents.

| Report | Source | What |
|--------|--------|------|
| **Rent roll** | leases + units + properties | Current tenancies: property, unit, tenant, rent, lease term, status, payment standing, balance — with a rent + balance total. Optional `property_id` / `portfolio_id` scope. |
| **T-12** | general ledger ([`accounting`](PAYMENTS.md)) | Trailing-twelve-month income statement for an LLC: each income/expense account by month, with monthly income/expense/NOI subtotals. Reuses `accounting::account_activity` per month. |
| **Aging** | outstanding `lease_payment`s | AR aging by age bucket (current / 1–30 / 31–60 / 61–90 / 90+) per tenant, with bucket + grand totals. |
| **Delinquency** | leases + `lease_payment`s | Tenants currently behind (`balance_cents > 0`): balance, days late (from the oldest outstanding charge), payment status. |
| **Owner statement** | settled payments + ledger | Cash-basis statement for one legal entity + period: rent collected − operating expenses (itemised by account) − management fee = **net owner draw**. Shares [`crate::payouts::gather_period`] with owner payouts, so a statement and the payout it explains always reconcile. |
| **1099 tax export** | vendor bills + settled rents | Annual information-return recipients ≥ $600: **1099-NEC** (nonemployee compensation to vendors/contractors, from paid `vendor_bill`s) and **1099-MISC** (Box 1 rents, gross rents collected per owning entity, with the entity's EIN). |

The three balance-based reports tie out: the rent roll's total balance equals
the aging grand total equals the delinquency total. Owner statements tie to the
`owner_payout` computation for the same entity + period.

---

## API

All under the `reports` module (JWT; tenant-scoped; self-gated on the module
being enabled), behind `report:read`:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/reports/rent-roll?property_id&portfolio_id` | Rent roll (JSON) |
| GET | `/reports/t12?entity=<llc>` | T-12 income statement for an LLC (JSON) |
| GET | `/reports/aging` | AR aging (JSON) |
| GET | `/reports/delinquency` | Delinquency (JSON) |
| GET | `/reports/owner-statement?entity=<llc>&from&to` | Owner statement for an entity + period (JSON; period defaults to month-to-date) |
| GET | `/reports/1099?year=<YYYY>` | 1099-NEC + 1099-MISC recipients for a year (JSON; defaults to last year) |
| GET | `/reports/<name>/export?format=csv\|pdf&…` | The same report as a downloadable CSV or PDF |

Exports stream a `text/csv` or `application/pdf` attachment (the PDF via the
same hand-rolled text→PDF writer the e-sign/lien-waiver flows use). An
unsupported `format` returns `400`.

---

## Operations analytics and the portfolio map

`api/src/routes/analytics.rs` (roadmap area 15, fix plan F20). All three read
live data and are narrowed to the properties the caller can see.

| Route | Gate | What |
|-------|------|------|
| `GET /analytics/operations?months=&property_id=` | `report:read` | Turns finished (count, average days from start to finish, average and total cost from the steps) and work orders opened (resolved, past their resolve target, average rating, average hours to resolve, spend) for each month and each property; work orders by kind; **repeat issues** (three or more of one kind at one property); and appliances whose repair spend in the window passed `analytics.replace_share_pct` (default 50) of their price, flagged **replace**. Also the turns and work orders open now. `months` is 1 to 36, default 12. |
| `GET /analytics/leasing?months=` | `report:read` | Tour requests, applications, approvals and leases from those applications in the window, the two conversion rates, and every listing's days on market (to the first lease off one of its applications, or to today), longest first. |
| `GET /portfolio/map` | `property:read` | Each property with its coordinates (from the property data), units and occupancy, open and urgent work orders, open turns, and its first site map. Properties with no coordinates are counted as unplaced; refreshing their property data geocodes them. |

The aggregations are pure functions with unit tests (`turn_stats`,
`ticket_stats`, `repeat_issues`, `appliance_spend`, `months_back`); the
integration scenario `analytics_and_map` checks a water heater with three
repairs worth 55% of its price comes back as a repeat issue and flagged to
replace.

## Frontend

`/console/reports` (nav: **Reports**) is a tabbed page: Operations · Leasing ·
Rent roll · T-12 · Aging · Delinquency · Owner statement · 1099. The tab is in
the address (`?tab=`). **Operations** has the period and property filters,
four figures (days to turn, cost to turn, past target, rating), monthly charts,
the replace-instead-of-repair table, repeat issues, work orders by kind, and a
table by property. **Leasing** has the funnel figures and the listings by days
on market. The standard reports render as tables with **CSV** and **PDF**
downloads; T-12 and Owner statement have an LLC picker and 1099 a year picker.

`/console/portfolio-map` (nav: **Portfolio map**) puts every property on one
map, coloured by occupancy (95% and up, 85 to 94%, under 85%) or by open work
(nothing open, work or a turn open, urgent work open), with a list and a detail
panel linking to the property, its work orders and its site map. The map is a
plain tile map (`components/map/TileMap.tsx`, math in `lib/tilemap.ts`) with no
library; tiles come from OpenStreetMap unless `NEXT_PUBLIC_MAP_TILES` names a
provider with a production plan (`{z}/{x}/{y}` placeholders). OpenStreetMap's
own servers are for light use only.
