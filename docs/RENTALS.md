# Rentals, Maintenance & Title

The operational layer for managing rentals and the complete title picture of a
property — everything you need to run a tenancy or evaluate a flip. Delivered as
three pluggable modules: **`rentals`**, **`maintenance`**, and **`title`** (all on
by default).

---

## Rentals (`rentals` module)

Units, leases/tenancies, and the rent ledger.

- `unit` — a rentable space in a property (number, beds/baths/sqft, market rent,
  status: occupied / vacant / make_ready / down).
- `lease` — a tenancy: tenant identity (inline), unit, rent, deposit, term,
  **status** (upcoming / active / notice / expired / ended) and **payment
  status** (current / late / partial) with an outstanding `balance_cents`.
- `lease_payment` — a rent-ledger entry (due date, amount, paid date, status).

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/properties/{id}/units` | `lease:read` | Units in a property |
| POST | `/properties/{id}/units` | `lease:manage` | Add a unit |
| PATCH | `/units/{id}` | `lease:manage` | Edit a unit |
| GET | `/leases?status=&property_id=` | `lease:read` | Lease/tenant directory |
| GET | `/properties/{id}/leases` | `lease:read` | Leases for a property |
| POST | `/properties/{id}/leases` | `lease:manage` | Create a lease |
| GET | `/leases/{id}` | `lease:read` | Lease + its payment ledger |
| PATCH | `/leases/{id}` | `lease:manage` | Edit a lease |
| GET | `/leases/{id}/payments` | `lease:read` | Rent ledger |
| POST | `/leases/{id}/payments` | `lease:manage` | Record a payment (updates balance + payment status) |

Recording a `paid` payment decrements the lease's `balance_cents` and flips its
`payment_status` to `current` (or `partial` if a balance remains).

---

## Maintenance (`maintenance` module)

Work-order tracking with assignment to either a platform member or an external
contractor (from the entities registry).

- `maintenance_ticket` — title, description, category, **priority**, **status**
  (open → triage → scheduled → in_progress → on_hold → resolved → closed),
  `assignee_user_id` **or** `assignee_entity_id`, optional unit/lease, cost.
- `ticket_comment` — the ticket timeline (comments + logged status changes).

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/tickets?status=&property_id=&priority=` | `maintenance:read` | Work-order board |
| GET | `/properties/{id}/tickets` | `maintenance:read` | Tickets for a property |
| GET | `/properties/{id}/maintenance` | `maintenance:read` | Maintenance tab: **open** work orders split from resolved **history**, with counts and open-work cost |
| POST | `/properties/{id}/tickets` | `maintenance:manage` | Open a ticket |
| GET | `/tickets/{id}` | `maintenance:read` | Ticket + timeline |
| PATCH | `/tickets/{id}` | `maintenance:manage` | Update status / assignee / fields (logs a status comment) |
| POST | `/tickets/{id}/comments` | `maintenance:manage` | Add a comment |

Residents open and follow their own requests (with photos) through the
`/my/tickets` portal routes — see [`PORTAL.md`](PORTAL.md). Phase 5 also
added **move-in/move-out inspections** (`/leases/{id}/inspections`, checklist
+ photos) and the **security-deposit disposition** (`/leases/{id}/deposit`)
to the rentals module, documented there. Phase 6 grew the module into the
**helpdesk** — SLA targets + breach scanning, contractor dispatch with
quotes → approval → vendor bill, preventive-maintenance plans, and the
auto make-ready turnover — see [`HELPDESK.md`](HELPDESK.md).

---

## Title & Ownership (`title` module)

The complete encumbrance + ownership picture — who owns the deed and who has a
claim — essential for both flips and rentals.

- `ownership` — deed holder(s): owner kind (llc / entity / individual / external),
  link to an LLC or counterparty, vesting, **ownership share** (`percent_bps`),
  deed type / recorded date / reference. Multiple rows model fractional ownership.
- `lien` — an encumbrance: lienholder (often a counterparty), kind (mortgage /
  tax / mechanics / judgment / hoa / other), amount, **position**, recorded date,
  status (active / released). The title-level view; financing detail lives in
  `mortgage` (see `docs/INVESTING.md`).

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/properties/{id}/ownership` | `title:read` | Deed holders |
| POST | `/properties/{id}/ownership` | `title:manage` | Add an owner |
| PATCH | `/ownership/{id}` | `title:manage` | Edit |
| DELETE | `/ownership/{id}` | `title:manage` | Remove |
| GET | `/properties/{id}/liens` | `title:read` | Liens / encumbrances |
| POST | `/properties/{id}/liens` | `title:manage` | Add a lien |
| PATCH | `/liens/{id}` | `title:manage` | Edit |
| DELETE | `/liens/{id}` | `title:manage` | Remove |

---

## Permissions

New permissions (`docs/IAM.md`): `lease:read`/`lease:manage`,
`maintenance:read`/`maintenance:manage`, `title:read`/`title:manage`. Granted to
workspace owners and property managers; back-office gets rentals + maintenance;
the maintenance persona gets maintenance manage; landlords get read-level
visibility.

## Frontend

- **Tenants/Leases** (`/console/leases`) — lease directory with status + payment
  badges.
- **Maintenance** (`/console/maintenance`) — the work-order board, filterable by
  status, with assignment and status changes.
- The **property profile** gains Units, Leases (tenant + status), open Tickets,
  and an Ownership & Liens section — the full property dossier. Its header shows
  the hero image beside the home/address/rental-status breakdown, and the
  Financials, Maintenance, and Documents tabs are each backed by a single
  aggregating endpoint (`/properties/{id}/financials|maintenance|documents`).

## Schema

Migration `m20240101_000009_rentals_title`: `unit`, `lease`, `lease_payment`,
`maintenance_ticket`, `ticket_comment`, `ownership`, `lien`. All tenant-scoped and
indexed by their parent.
