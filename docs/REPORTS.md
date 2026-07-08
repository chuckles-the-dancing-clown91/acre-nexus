# Standard PM Reports

The reports every property manager expects on day one (roadmap Phase 8, issue
#56), delivered as the pluggable **`reports`** module (`docs/MODULES.md`). Each
report reads live off the shipped rentals + general-ledger data — no separate
reporting store — and is viewable in the console and exportable to **CSV or
PDF**. Gated by `report:read`. Money is integer cents.

| Report | Source | What |
|--------|--------|------|
| **Rent roll** | leases + units + properties | Current tenancies: property, unit, tenant, rent, lease term, status, payment standing, balance — with a rent + balance total. Optional `property_id` / `portfolio_id` scope. |
| **T-12** | general ledger ([`accounting`](PAYMENTS.md)) | Trailing-twelve-month income statement for an LLC: each income/expense account by month, with monthly income/expense/NOI subtotals. Reuses `accounting::account_activity` per month. |
| **Aging** | outstanding `lease_payment`s | AR aging by age bucket (current / 1–30 / 31–60 / 61–90 / 90+) per tenant, with bucket + grand totals. |
| **Delinquency** | leases + `lease_payment`s | Tenants currently behind (`balance_cents > 0`): balance, days late (from the oldest outstanding charge), payment status. |

The three balance-based reports tie out: the rent roll's total balance equals
the aging grand total equals the delinquency total.

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
| GET | `/reports/<name>/export?format=csv\|pdf&…` | The same report as a downloadable CSV or PDF |

Exports stream a `text/csv` or `application/pdf` attachment (the PDF via the
same hand-rolled text→PDF writer the e-sign/lien-waiver flows use). An
unsupported `format` returns `400`.

---

## Frontend

`/console/reports` (nav: **Reports**) is a tabbed page — Rent roll · T-12 ·
Aging · Delinquency — each rendering the report as a table with **CSV** and
**PDF** download buttons (fetched as an authenticated blob). T-12 has an LLC
selector. Northwind's demo data (leases, ledger, outstanding payments) makes all
four reports populate out of the box.
