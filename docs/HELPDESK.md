# Helpdesk & Maintenance Operations (Phase 6)

The support-desk layer on top of the maintenance module — roadmap Phase 6,
issue #10. Turns internal tickets into a full support + vendor-ops loop:
resident request → SLA-tracked triage → contractor dispatch → quote →
approval → invoice → payment → property ledger.

Resident-facing ticketing and threaded comms shipped with Phase 5
([`PORTAL.md`](PORTAL.md)); this phase adds the operations machinery around
them. All of it lives in the existing `maintenance` module.

## SLA policy

Per-priority first-response and resolution targets, configured per workspace
(console → Settings → Helpdesk) as `priority:hours` pairs:

| Key | Default |
| --- | --- |
| `helpdesk.sla_response_hours` | `urgent:4,high:8,normal:24,low:72` |
| `helpdesk.sla_resolve_hours` | `urgent:24,high:72,normal:168,low:336` |

Targets are stamped onto the ticket at create (`sla_response_due_at` /
`sla_resolve_due_at`) — every door stamps them: the staff console, the
resident portal, plan-generated tickets, and turnover tickets. A priority
change re-stamps whichever targets are still open, measured from creation.
`0` (or omitting a priority) disables that target.

Lifecycle timestamps close the loop:

- **`first_response_at`** — the first staff touch: a comment, a status move,
  or an assignment.
- **`resolved_at`** — stamped entering `resolved`/`closed`, cleared on
  reopen.

The DTO derives a state per target at read time — `none` / `on_track` /
`met` / `breached` (a target completed late stays `breached`) — surfaced as
badges on the board and the ticket detail's SLA panel.

## The helpdesk scan

One durable, self-rescheduling `helpdesk_scan` job per tenant (the
billing-cycle/reminder-scan pattern; interval `helpdesk.scan_interval_secs`,
default hourly, ensured at boot and at tenant provisioning). Each pass:

1. **Breach notifications** — open tickets past a target notify everyone
   holding `maintenance:read` (`ticket_sla_breached` template) through the
   in-app inbox + push + chat. The substrate's idempotency key (ticket +
   target kind) makes each breach fire exactly once.
2. **Preventive plans** — every active `maintenance_plan` whose
   `next_due_date` arrived opens its ticket (reporter "Preventive
   maintenance", SLA stamped, staff notified) and advances past today by
   whole cadences — a scan that was down for weeks generates one ticket,
   not a backlog.

## Intake context, replies & internal notes

The full-maintenance round-out finished the ticket itself:

- **Location & access**: every ticket carries `location` (where in the home),
  `access_notes` (lockbox, pets, alarm, best times) and
  `permission_to_enter` — collected in the resident portal's request form
  and surfaced as an entry banner on the console detail page, so a
  dispatcher never sends a contractor in blind.
- **Public replies vs. internal notes**: `ticket_comment.visibility` splits
  the timeline. A **public reply** (`POST /tickets/{id}/comments`, default)
  shows in the resident's portal with the author's name and emails them
  (`maintenance_reply` template). An **internal note**
  (`visibility: "internal"`) is staff-only — the portal filters it out, and
  it renders highlighted in the console. System entries follow the same
  rule: vendor-bill payments log internally (money detail stays staff-side);
  inbound email replies land internally for staff to relay; status changes
  stay public.

## Equipment registry (assets)

`asset` registers the serviceable equipment — AC units, water heaters,
appliances and other utilities — per property (optionally per unit): kind
(`hvac` / `appliance` / `plumbing` / `electrical` / `safety` / `structural`
/ `other`), make/model/serial, install date, and **warranty tracking**
(`none` / `active` / `expired` derived at read time). Manuals, warranty
docs, and photos file against the asset through the document service
(`owner_type = "asset"`).

- `GET /assets?property_id&unit_id&status` (`maintenance:read`) ·
  `POST /assets` · `PATCH /assets/{id}` (edit / retire,
  `maintenance:manage`).
- Tickets attach the asset being serviced (`asset_id`, validated against
  the ticket's property); the console detail page picks from the property's
  registry and shows the asset chip, and the property profile gains an
  **Equipment & assets** card.

## Parts, costs & the stockroom

- **Ticket lines** (`ticket_line`): itemized `part` / `labor` / `fee` /
  `other` entries on a work order — description, quantity, unit cost. When
  any lines exist their totals are the **single source of the ticket's
  cost** (the vendor-bill prefill reads it): approving a quote lands the
  quoted amount as a labor line, and a manual `cost_cents` PATCH is
  rejected — edit the lines instead. `POST /tickets/{id}/lines` ·
  `DELETE /ticket-lines/{id}` (`maintenance:manage`).
- **Inventory** (`inventory_item`): the stockroom — SKU, quantity on hand,
  unit cost, reorder level, storage location, and a **serial-number pool**
  for serialized stock. A part line pulled from inventory validates stock
  (under a row lock — concurrent pulls can't oversell or take the same
  serial), decrements it, and consumes the chosen serial; deleting the line
  restocks (and returns the serial). `GET/POST /inventory` ·
  `PATCH /inventory/{id}` (restock/correct/archive). The helpdesk scan
  raises a **low-stock alert** (`inventory_low` template) once per episode:
  when quantity falls to/below the reorder level, with the alert re-armed
  once restocking lifts it back above.

## Waiting-on discipline & follow-ups

Putting a ticket **on hold** demands a reason: `waiting_on`
(`parts` / `vendor` / `resident` / `owner` / `other`), a `follow_up_date`,
and a follow-up note (logged as an internal timeline entry) — enforced
server-side (the invariant is `on_hold` ⇔ a waiting reason is set: no
parking without a reason, no waiting tag on an active ticket), prompted
inline in the console. The helpdesk scan chases the date: when it arrives,
staff get a `ticket_follow_up` notification (once per ticket per date —
re-dating the follow-up re-arms it). Leaving on-hold clears the waiting
state.

## Resident updates & reviews

- **Updates pushed and emailed**: every resident-facing event — staff public
  replies (`maintenance_reply`) and status changes (`maintenance_update`) —
  now goes through `notify::notify_person`: email always, plus the in-app
  inbox and web push when the address belongs to a portal account.
- **Reviews**: once a request is resolved/closed the resident rates it from
  the portal (1–5 stars + optional comment, once per ticket —
  `POST /my/tickets/{id}/review`). The rating shows on the console ticket
  (badge + review card) and staff are notified (`ticket_reviewed`).

## Contractor dispatch

- **Assignment notifications** (`PATCH /tickets/{id}`): assigning a member
  notifies them in-app + by email (`ticket_assigned`); assigning an external
  contractor (counterparty with an email) sends the dispatch email
  (`ticket_dispatch`) with property, priority, scheduled date, and scope.
- **Scheduling**: the ticket's `due_date` doubles as the scheduled visit
  date and rides along in dispatch emails.
- **Quotes → approval**: `ticket_quote` records a contractor's bid
  (`POST /tickets/{id}/quotes`, `maintenance:manage`; contractor defaults to
  the ticket's assignee). Approving (`POST /ticket-quotes/{id}/approve`,
  gated by `payable:approve` — the same people who approve vendor bills)
  lands the quoted amount as a labor line — so the ticket's cost includes
  it alongside any parts — and attaches the contractor if the ticket had
  none. Rejection just closes the quote.
- **Invoice → payment**: the Phase 3 accounts-payable loop finishes the job —
  `POST /payables { maintenance_ticket_id }` prefills the vendor (the
  ticket's contractor), property, amount (the approved quote), and memo;
  approval accrues `Dr Property Expenses / Cr Accounts Payable` and payment
  rides the provider payout rail (see
  [`PAYMENTS.md`](PAYMENTS.md#accounts-payable-vendor-bills)). Resolution +
  cost land on the property's books.

## Preventive maintenance & turnover

- **Plans** (`maintenance_plan`): a recurring task (HVAC service, gutter
  cleaning, detector checks) per property (optionally a unit) with a
  category/priority, `cadence_days`, and `next_due_date`. CRUD at
  `GET/POST /maintenance-plans` + `PATCH /maintenance-plans/{id}`
  (`maintenance:read` / `maintenance:manage`); pause/resume via `active`.
- **Make-ready / turnover**: completing a **move-out inspection** (Phase 5)
  auto-opens a high-priority "Turnover / make-ready" ticket on the unit and
  flips the unit's status to `make_ready` — gated by the
  `helpdesk.auto_turnover` setting (default on).

## External connector (deferred)

The optional Zendesk/Intercom sync from the epic remains deferred: the
provider framework (Phase 1) is the natural home when a client actually
runs an external desk, and nothing here precludes it.

## Console

- The **maintenance board** shows an "SLA breached" flag per row and links
  each ticket to its detail page.
- **Ticket detail** (`/console/maintenance/{id}`): SLA panel (both targets
  with due/met/breached state), triage & dispatch controls (status,
  priority, member + contractor assignment, scheduled date), the comment
  timeline, the quotes card (record / approve / reject), a one-click
  **Create vendor bill** on a resolved ticket with an approved cost, and
  attachments (resident photos land here too).
- A **Preventive maintenance** card on the board manages plans.

## Endpoints

| Method | Path | Permission |
|--------|------|-----------|
| POST | `/tickets/{id}/lines` | `maintenance:manage` |
| DELETE | `/ticket-lines/{id}` | `maintenance:manage` |
| GET | `/inventory` | `maintenance:read` |
| POST | `/inventory` | `maintenance:manage` |
| PATCH | `/inventory/{id}` | `maintenance:manage` |
| GET | `/assets` | `maintenance:read` |
| POST | `/assets` | `maintenance:manage` |
| PATCH | `/assets/{id}` | `maintenance:manage` |
| POST | `/tickets/{id}/quotes` | `maintenance:manage` |
| POST | `/ticket-quotes/{id}/approve` | `payable:approve` |
| POST | `/ticket-quotes/{id}/reject` | `payable:approve` |
| GET | `/maintenance-plans` | `maintenance:read` |
| POST | `/maintenance-plans` | `maintenance:manage` |
| PATCH | `/maintenance-plans/{id}` | `maintenance:manage` |

Quotes ride along on `GET /tickets/{id}`.

## Audit, templates, schema

- Audit actions: `ticket_quote.add/approve/reject`,
  `maintenance_plan.create/update/run`, `asset.create/update`,
  `ticket_line.add/remove`, `inventory_item.create/update`, `ticket.review`
  (plus the existing `ticket.*`).
- Templates: `ticket_assigned`, `ticket_dispatch`, `ticket_sla_breached`,
  `maintenance_reply`, `ticket_follow_up`, `inventory_low`,
  `ticket_reviewed`.
- Migrations: `m20240101_000032_helpdesk` (four SLA/lifecycle columns on
  `maintenance_ticket`, plus `ticket_quote` and `maintenance_plan`) and
  `m20240101_000033_maintenance_full` (the `asset` registry; ticket
  `location`/`access_notes`/`permission_to_enter`/`asset_id`; comment
  `visibility`/`author_name`) and `m20240101_000034_maintenance_ops`
  (`inventory_item`, `ticket_line`; ticket
  `waiting_on`/`follow_up_date`/`rating`/`review_comment`/`reviewed_at`) —
  tenant-scoped, RLS-enforced.

## Definition of Done (met)

A resident opens a ticket from the portal (Phase 5), it routes to a
contractor with an SLA (dispatch email + stamped targets, breaches
surfaced by the scan), and resolution + cost flow back to the property
ledger (approved quote → prefilled vendor bill → approval accrual →
payment).
