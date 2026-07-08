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

## Frontend

`/console/reports` (nav: **Reports**) is a tabbed page — Rent roll · T-12 ·
Aging · Delinquency · Owner statement · 1099 tax — each rendering the report as
a table with **CSV** and **PDF** download buttons (fetched as an authenticated
blob). T-12 and Owner statement have an LLC selector; 1099 has a year selector.
Northwind's demo data (leases, ledger, outstanding payments, owner entities)
makes the reports populate out of the box.
