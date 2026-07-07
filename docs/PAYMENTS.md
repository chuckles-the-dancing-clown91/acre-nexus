# Payments & Accounting Core (Phase 3)

How money moves through Acre Nexus: the double-entry general ledger, rent
collection through Stripe (cards/ACH with saved methods + autopay), the
late-fee engine, Plaid bank feeds + reconciliation, owner payouts with
statements, and the financial dashboards. Roadmap Phase 3, issues #33–#39
under epic #7.

Everything here is one pluggable module (`accounting`, on by default) built on
the Phase 1 substrate: providers are **sandbox-first** (deterministic
simulations by default, live via `LIVE_PROVIDERS`), inbound webhooks ride the
signature-verified ingestion endpoint, all async work rides the durable
retrying job queue, and every state change writes a domain audit event.

## The double-entry ledger

The general ledger is partitioned by **legal entity** (`llc.id`): each LLC
keeps its own books. Three tables:

| Table | What it is |
| --- | --- |
| `ledger_account` | Chart of accounts per entity. Seeded system accounts carry a stable `subtype` the posting rules resolve by; `is_trust` marks escrow accounts. |
| `ledger_txn` | One balanced transaction: date, memo, and a `source_type`/`source_id` tying it to the domain event (payment, late fee, payout, manual entry) that produced it. |
| `ledger_entry` | One debit or credit leg (always positive; `side` says which way), with optional property/lease dimensions for reporting. |

### Default chart of accounts

Seeded idempotently the first time an entity's books are touched
(`accounting::ensure_chart`); tenants can add custom accounts on top.

| Code | Account | Kind | Subtype |
| --- | --- | --- | --- |
| 1000 | Operating Bank | asset | `operating_bank` |
| 1100 | Trust Bank (Escrow) | asset | `trust_bank` (trust) |
| 1200 | Accounts Receivable | asset | `accounts_receivable` |
| 2100 | Security Deposits Held | liability | `security_deposits` (trust) |
| 3000 | Owner Equity | equity | `owner_equity` |
| 3100 | Owner Draws | equity | `owner_draws` |
| 4000 | Rental Income | income | `rent_income` |
| 4100 | Late Fee Income | income | `late_fee_income` |
| 4200 | Other Fee Income | income | `fee_income` |
| 5000 | Property Expenses | expense | `property_expenses` |
| 5100 | Management Fees | expense | `management_fees` |

### The single posting path

`accounting::post` is the only write path into the ledger. Before anything is
written, `validate_legs` (pure, unit-tested) enforces:

1. at least two legs, every amount strictly positive;
2. **debits equal credits** — a trial balance always sums to zero;
3. every account belongs to the posting's entity — one transaction never
   spans two entities' books, which rules out cross-entity commingling by
   construction (`assert_no_commingling` remains the guard for any future
   inter-entity transfer flow);
4. **trust integrity** — the signed movement of trust *asset* accounts must
   equal the signed movement of trust *liability* accounts, so escrow cash
   only ever moves against what is owed back. Trust funds can neither leak
   into operating cash nor absorb operating shortfalls.

### Standard posting rules

| Domain event | Entry |
| --- | --- |
| Rent falls due | `Dr Accounts Receivable / Cr Rental Income` |
| Rent or fee settles | `Dr Operating Bank / Cr Accounts Receivable` |
| Security deposit settles | `Dr Trust Bank / Cr Security Deposits Held` |
| Late fee assessed | `Dr Accounts Receivable / Cr Late Fee Income` |
| Owner payout settles | `Dr Owner Draws (net) + Dr Management Fees / Cr Operating Bank` |
| Manual journal entry | any balanced set of legs (`ledger:manage`) |

### Reports

- `GET /accounting/accounts?entity=` — chart of accounts with activity +
  normal-direction balances (seeds the default chart on first read).
- `GET /accounting/transactions?entity=` — the journal, newest first.
- `POST /accounting/transactions` — manual journal entry (`ledger:manage`).
- `GET /accounting/trial-balance?entity=` — per-account debit/credit totals
  plus a `balanced` flag.
- `GET /accounting/income-statement?entity=&from=&to=` — income vs expenses
  for a period.
- `GET /accounting/trust-reconciliation?entity=` — escrow cash vs deposit
  liability; `reconciled` when they're equal.

## Rent collection

A `lease_payment` row is both the **receivable** ("July rent is due") and the
**payment attempt** against it: `due → processing → paid | failed` (plus
`late` once past grace). The electronic flow:

1. The resident (or autopay) pays a due item: the row flips to `processing`
   and a durable `payment_process` job is enqueued.
2. The job charges through the Stripe provider (`providers/payments.rs`) —
   an off-session PaymentIntent against the saved method token. Simulated
   mode mints deterministic `sim_pi_…` ids; a method whose token ends in
   `0002` (the canonical Stripe decline test number) declines, so the
   failure path is demoable.
3. **Settlement** lands in `payments::settle_payment` — from the Stripe
   webhook (`payment_intent.succeeded` / `payment_intent.payment_failed`) in
   live mode, or after `payments.callback_delay_secs` in simulation. The one
   settlement path updates the lease's balance and standing, posts the
   ledger entry, stores a **receipt PDF** in the document service
   (`RCT-…` numbers), audits `payment.settle`, emails the resident, and
   notifies staff.

Failures are first-class: the payment keeps its `failure_reason`, the
resident is emailed, and autopay never re-charges a failed receivable — the
resident retries manually.

### Saved methods & autopay

`payment_method` stores **provider tokens only** (`pm_…` live via client-side
tokenization, `sim_pm_…` simulated) plus display metadata — PANs and account
numbers never touch the platform. At most one active autopay method per lease
(partial unique index); enrollment picks the day of month (1–28). The renter
portal (`/my/payment-methods`, `/my/autopay`) manages both; staff see methods
read-only at `GET /leases/{id}/payment-methods`.

### The billing cycle

One self-rescheduling `billing_cycle` job per tenant (ensured at boot and at
tenant provisioning) runs every few hours and, idempotently:

1. **raises rent receivables** — on the tenant's `payments.rent_due_day`,
   each active lease gets its month's receivable (base rent + recurring
   lease charges), accrued to the ledger;
2. **assesses late fees** — past `payments.late_fee_grace_days`, the
   receivable flips `late`, the lease drops to late standing, and the
   configured fee (flat + percentage, `one_time` or `daily`, capped) lands
   three ways: a `lease_charge` documenting its origin, a payable
   receivable, and the ledger accrual — plus a resident email;
3. **runs autopay** — due rent charges through each lease's enrolled method;
4. **refreshes bank feeds** roughly daily;
5. **captures the monthly snapshot** for dashboard history.

## Bank feeds & reconciliation

Linking a `bank_account` (Plaid live via a Link `public_token`; simulated
otherwise) sets its provider account id; `bank_feed_sync` jobs then pull
transactions (deduped by provider id on re-sync) and **auto-match** deposits
against settled payments — exact amount, dates within 3 days, one payment per
line. What doesn't match stays `unmatched` for the console's manual
match/ignore review queue; withdrawals never auto-match. The simulated feed
returns exactly the deposits the ledger expects plus deterministic noise (a
service fee, an unrelated deposit) so reconciliation always has both hits and
leftovers to demonstrate.

Endpoints: `GET /bank-accounts`, `POST /bank-accounts/{id}/link`,
`POST /bank-accounts/{id}/sync`, `GET /bank-accounts/{id}/transactions`,
`POST /bank-transactions/{id}/match`, `POST /bank-transactions/{id}/ignore`.

## Owner payouts

`POST /payouts/compute` builds a draft from one entity's actual books for a
period: **rent collected** (settled payments on the entity's properties) −
**operating expenses** (expense-account postings, excluding management fees)
− the **management fee** (`payments.mgmt_fee_bps` of rent collected).
`POST /payouts/{id}/execute` runs it as an ACH transfer through the payments
provider; settlement (webhook live, immediate in simulation) posts the draw
to the ledger, stores a generated **owner statement PDF** against the entity,
audits, and notifies staff. Failed payouts keep their reason and can be
re-executed.

## Accounts payable (vendor bills)

Issue #58 closes the loop from "contractor did the work" to "contractor got
paid". A `vendor_bill` ties a vendor (`counterparty`) — and optionally the
completed `maintenance_ticket` — to an amount with line items, on one
entity's books:

```
draft → submitted → approved → processing → paid
          ↓ reject      (failed retries; void only before approval)
        draft
```

- **Create** (`POST /payables`, `payable:manage`) — passing
  `maintenance_ticket_id` prefills the vendor (the ticket's contractor
  assignee), property, amount (`cost_cents`), and memo, so a completed work
  order becomes a bill in one call. The entity is the property's owning LLC
  unless passed explicitly.
- **Submit** (`payable:manage`) hands the bill to the approvers — everyone
  holding `payable:approve` is notified (`vendor_bill_submitted`).
- **Approve** (`payable:approve`) accrues the expense immediately:
  `Dr Property Expenses / Cr Accounts Payable` (the chart gained system
  account `2000 Accounts Payable`). The books recognize the cost when the
  obligation is committed, not when cash moves — and because it posts to
  `property_expenses`, approved bills flow into NOI and payout expense
  computations automatically. Reject returns the bill to draft with a
  reason.
- **Pay** (`payable:approve`) rides the payments provider's payout rail on
  the durable queue (`vendor_bill_pay` job; sandbox by default, ACH live).
  Settlement — webhook-driven live (`payout.paid`/`payout.failed` match the
  bill before owner payouts), immediate in simulation — posts
  `Dr Accounts Payable / Cr Operating Bank`, stamps `payment_txn_id`,
  records the cost + a timeline note on the originating ticket, notifies
  staff, and emails the vendor a remittance advice
  (`vendor_bill_remittance`) when the counterparty has an email.

Console: **`/console/payables`** — list with status filter, create dialog,
and role-gated Submit / Approve / Reject / Pay / Void actions
(`property_manager` submits; `back_office` and the workspace owner approve
and pay; `landlord` reads).

Every transition audits (`vendor_bill.create/submit/approve/reject/pay/settle/void`).

## Renter portal

`/account/payments` in the app, backed by:

- `GET /my/lease` — the signed-in resident's lease (matched by account
  email, like `/my/applications`): balance, payable items, deposit status,
  saved methods, autopay state, and receipt history.
- `POST /my/payments` — pay a due item in full, or `{"kind": "deposit"}` to
  raise + pay the security deposit into trust.
- `POST /my/payment-methods` / `DELETE /my/payment-methods/{id}`.
- `PUT /my/autopay` / `DELETE /my/autopay`.

No staff permission is required — everything is scoped to the resident's own
lease. Phase 5 widened the portal beyond payments (lease + documents,
maintenance, messaging, deposit disposition) — see [`PORTAL.md`](PORTAL.md).

### Security-deposit disposition (Phase 5)

At move-out the deposit held in trust settles through a **disposition**:
itemized deductions post `Dr Security Deposits Held + Dr Operating Bank / Cr
Trust Bank + Cr Other Fee Income` (one balanced transaction — the trust
invariant holds), the remainder refunds to the resident on the same provider
payout rail as owner draws (`deposit_refund` job; the `payout.*` webhook
matches bills → deposit refunds → owner draws), and settlement posts
`Dr Security Deposits Held / Cr Trust Bank`, files a statement PDF on the
lease, and emails the resident. Full design in
[`PORTAL.md`](PORTAL.md#security-deposit-disposition).

## Financial dashboards

`GET /finance/series?months=N` powers the console dashboard's trend charts:
rent due/collected and NOI are computed live from the payments table and the
ledger per month; occupancy, delinquency, and portfolio value come from the
monthly `financial_snapshot` history captured by the billing cycle (the
current month is always computed fresh). The frontend renders these with a
dependency-free SVG `TrendChart` driven by the theme's CSS variables.

## Configuration

Workspace settings (console → Settings → Payments), all audited:

| Key | Default | Meaning |
| --- | --- | --- |
| `payments.autopay_enabled` | `true` | Residents may enroll in autopay |
| `payments.rent_due_day` | `1` | Day of month rent falls due (1–28) |
| `payments.callback_delay_secs` | `5` | Simulated processor confirm delay |
| `payments.late_fee_grace_days` | `5` | Days past due before a late fee (0 = never) |
| `payments.late_fee_flat_cents` | `7500` | Flat late-fee component |
| `payments.late_fee_percent_bps` | `0` | Percentage component (bps of overdue) |
| `payments.late_fee_recurrence` | `one_time` | `one_time` or `daily` |
| `payments.late_fee_max_cents` | `0` | Cap per billing period (0 = none) |
| `payments.mgmt_fee_bps` | `800` | Management fee withheld from payouts |

Going live: store `stripe.secret_key` (and `plaid.client_id` /
`plaid.secret`) in the credential vault, configure the webhook signing secret
under `webhook.stripe.secret` (Stripe events arrive at
`POST /webhooks/stripe`, Plaid at `POST /webhooks/plaid`), and list the
providers in `LIVE_PROVIDERS`. Until then everything runs simulated — the
default, and what CI and the demo seed exercise.

## Permissions & audit

New permissions: `ledger:read`, `ledger:manage`, `payment:read`,
`payment:manage`, `payout:manage` — granted to the workspace-owner and
back-office roles (property managers get ledger/payment read + payment
manage; landlords get read-only books).

New audit actions: `ledger.post`, `ledger_account.create`, `payment.create`,
`payment.settle`, `payment_method.add/remove`, `autopay.enroll/cancel`,
`late_fee.apply`, `payout.create/execute/settle`, `bank_account.link`,
`bank_feed.sync`, `bank_txn.match`.

## Demo data

The seed gives Northwind a full year of books on Maple Holdings LLC: 11
months of settled rent (with balanced accrual + settlement postings and
receipts), security deposits held in trust (trust reconciliation shows $0
difference), monthly operating expenses, a linked bank account whose feed
auto-matches, an executed owner payout with statement + a draft one ready to
execute, monthly snapshots for the charts, and a resident portal login —
`taylor@example.com` / `password` — with a saved card enrolled in autopay
(watch the billing cycle collect the current month's rent on boot). Jordan
Avery's lease is the delinquency story: last month unpaid, late fee assessed,
and a declining test card on file.
