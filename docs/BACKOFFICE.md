# The back office

Vantedge roadmap phase 2B — Alpha Power Wash's back office, translated to
property management. The point isn't the list of features; it's that **every
number comes from the same rows**, so payroll, work-order costs, owner bills
and tax files always agree with each other.

```
 clock in on a work order ─┐
 miles / receipts / parts ─┼─► approved time + costs ─┬─► payroll week (overtime rule) ─► Gusto / CSV / PDF
                           │                           ├─► work-order cost: pay + OT share + burden
                           │                           │     + parts + mileage + expenses (+ vendor bills)
                           │                           ├─► bill to owner: hours × bill rate + parts & receipts
                           │                           │     + markup → AP bill on the owner's LLC
                           │                           │     → approve posts to their ledger → owner statement
                           │                           └─► profit by work order / property / technician /
                           │                                 category / month; tax package
```

## People

`employee_profile` (1:1 with a user in a workspace): title, employment type
(`full_time` / `part_time` / `seasonal` / `contractor` = 1099), **pay rate**,
**bill rate** (what an hour of their work charges owners), hire / end date,
weekly target, default vehicle, mileage paid back, emergency contact, calendar
colour, notes. Pay and bill rates are only visible and editable with
`payroll:read`.

| Permission | What it opens |
| --- | --- |
| `team:read` | roster, schedules, time off, timesheets, work-order bill previews, dashboard |
| `team:manage` | edit profiles, shifts, review time off, correct / approve time, settle missed punches, bill owners |
| `payroll:read` | pay and bill rates, payroll, profit / margins, tax package, Gusto |
| `expense:read` / `expense:manage` | everyone's expenses and receipts / change and pay them back |

Anyone with an employee profile uses the self-service routes (`/me/*`) without
any of these — clock in, fix their own unapproved time, log miles and
receipts, ask for time off. Built-in roles pick up new permissions on boot.

## The clock

`time_entry` is logged against a **work order**, a **rehab project**, a
**property**, or travel / shop / office / other time.

- One open entry per person (partial unique index); starting new work closes
  the old; overlapping manual entries are refused; breaks come off.
- Pay and bill rates are **frozen when the entry closes** — a raise never
  rewrites last month's payroll or an owner bill.
- **Missed punches** (every 15 minutes, and whenever timesheets are opened): an
  entry running past `workforce.missed_punch_hours` (default 12), or whose work
  order was resolved over an hour ago, is closed at the best evidence (the
  resolved time, else start + 8 h), flagged, and held from payroll and approval.
  The technician says when they really finished; the office settles it.
- **Approval**: the office approves (one or many); approved time is locked for
  the technician; time on a live owner bill is locked for everyone until that
  bill is voided.
- **Location** (off by default): noted at clock-in / clock-out only, measured
  from the property, flagged beyond the radius, never blocking.
- A Monday note tells `team:manage` holders about time from before this week
  still waiting for approval.

## Overtime

Ported rule-for-rule from Alpha (`workforce::overtime`), in whole minutes:

- **weekly** (FLSA): 1.5× over 40 hours in a Monday–Sunday week;
- **california** (Labor Code §510): also 1.5× over 8 and 2× over 12 hours in a
  day; on the seventh consecutive day worked the first 8 hours are 1.5× and the
  rest 2×; daily overtime doesn't count again toward the 40.

Entries belong to the local day they started (`workforce.timezone`). 1099
contractors are straight time. Reports split whole weeks only.

## Expenses & mileage

`expense`: date, category, vendor, amount, deductible, vehicle (company /
personal), reimbursable, **billable to owner**, and the work order / rehab
project / property / asset it's for. Mileage is priced at
`workforce.mileage_rate_mills` (IRS standard, default $0.70) from miles or
odometer readings (round trips doubled); own-vehicle trips are paid back when
the person's profile says so. Receipts are documents (`owner_type = expense`)
uploaded through a signed URL.

## Costing and billing owners

For a work order or rehab project (`GET /costs/{work-orders|rehab-projects}/{id}`):

- **labor** = time at frozen pay rates + its share of each person's weekly
  overtime premium (the whole week split, the premium spread over the week's
  entries by minutes) + labor burden % (not on contractors);
- **parts** (work-order part lines, at cost), **other line items**, **mileage**,
  **expenses**; optional **overhead** per labor hour for a net figure;
- **billed** = live in-house bills on it; **unbilled** = what billing would
  charge now; **gross**, **gross %**, **net**; outside vendor bills shown as the
  owner's cost; the **bill rate that would reach the target margin**.

`POST …/bill-owner` turns the preview into a draft AP bill from **In-house
maintenance** (a counterparty created on first use) on the property's LLC:
one labor line per person and rate (approved time only — the rest is listed as
held back), parts and billable expenses with `workforce.maintenance_markup_bps`,
line items at face. It then follows the normal submit → approve (posts
`Dr Property Expenses / Cr Accounts Payable` to the owner's books) → pay flow
and lands on the owner statement.

## Reports (JSON + CSV / PDF)

| Report | Route | Needs |
| --- | --- | --- |
| Payroll — per person-week: days, entries, hours, regular, OT, DT, rate, gross, mileage paid back; approved-only with what was left out | `/reports/payroll` | `payroll:read` |
| Timesheets | `/reports/timesheets/export` | `team:read` |
| Profit — every work order / project with activity, rolled up by property, technician, category, month, with KPIs | `/reports/profit` | `payroll:read` |
| Tax package — summary, expenses by category, pay by person (W-2 / 1099-NEC), mileage log, expense ledger, missing receipts, key federal + California dates | `/reports/taxes?year&quarter` | `payroll:read` |
| Back-office dashboard | `/backoffice/dashboard` | `team:read` |
| Cost sheet for one work order / project | `/costs/{kind}/{id}/sheet.pdf` | `team:read` (+ margins with `payroll:read`) |

Every PDF comes from `pdfdoc` — Helvetica with real glyph widths, tables with
repeated headers and right-aligned money, landscape for wide tables, the
workspace name and "Page n of N" on every page. The older reports (rent roll,
T-12, aging, delinquency, owner statements, 1099) use it too.

## Gusto

Push only, as in Alpha: `GET /payroll/gusto/hours` (and `.csv`) shows what
would go — whole weeks ending in the pay period, approved time only —
and `POST /payroll/gusto/push` fills an unprocessed Gusto payroll with
"Regular Hours" / "Overtime" / "Double overtime" per employee matched by email.
Sandbox-first: simulated unless `LIVE_PROVIDERS` lists `gusto`; the token lives
in the vault as `gusto.access_token`, the company in
`payroll.gusto_company_uuid`. Gusto still runs the payroll.

## CRM

Owners are the property manager's clients (`entity:read` / `entity:manage`):

- **Owner directory** — LLCs, properties, doors, last contact, open follow-ups.
- **Timeline** (`crm_note`) — notes / calls / emails / meetings / issues / texts
  about owners, owner leads, vendors and properties; pinned; **follow-ups**
  that stay open until done (`/crm/follow-ups`).
- **Owner-lead pipeline** (`owner_lead`) — new → contacted → proposal → won /
  lost, by source, with doors, estimated rent and the fee; stage moves logged;
  weighted pipeline and win rate by source; a printable **management proposal**;
  convert-to-owner carries the history over.

## Settings (Team & payroll)

`workforce.timezone`, `workforce.overtime_rule`, `workforce.labor_burden_bps`,
`workforce.overhead_per_hour_cents`, `workforce.target_margin_bps`,
`workforce.mileage_rate_mills`, `workforce.maintenance_markup_bps`,
`workforce.missed_punch_hours`, `workforce.clock_location`,
`workforce.clock_location_radius_m`, `payroll.gusto_company_uuid`.

## Not yet (from Alpha)

QuickBooks Online sync (Vantedge keeps its own ledger, so this is an export
choice), equipment meters and service schedules on assets, inventory stock
movements with weighted-average cost, compliance documents (the firm's
insurance, licenses, vendor COIs / W-9s) with expiry reminders, tips (not a
property-management need), and offline punches from an installable app.
