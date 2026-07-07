# Resident Portal & Tenant Lifecycle (Phase 5)

The renter-facing side of Acre Nexus, end to end: apply → screened → signed →
autopay → live in the portal → move out with a settled deposit. Roadmap
Phase 5, issue #9. This document covers the **portal round-out** and the
**move-in/move-out lifecycle** shipped in this phase; the earlier slices are
documented where they were built (applications in
[`LEASING.md`](LEASING.md), pay-rent in [`PAYMENTS.md`](PAYMENTS.md#renter-portal)).

## The `/my/*` convention

Renters hold no console permissions (the `renter` role deliberately carries
only vehicle self-service — see [`IAM.md`](IAM.md)). Every portal surface is
an unguarded `/my/*` route scoped to **the signed-in resident's own lease**,
resolved by matching the account email against `lease.tenant_email`
(`payments::lease_for_user`). Document and deposit reads also accept the
resident's most recent **past** lease, so a moved-out resident can still
download their statement.

## Portal surfaces

### My lease + documents (`/account/lease`)

- `GET /my/lease` — lease summary (term, rent, deposit, balance, standing;
  extended this phase with `tenant_name`, `start_date`, `end_date`).
- `GET /my/documents` — everything filed against the lease: the signed lease
  PDF (Phase 2 e-sign), payment receipts (Phase 3), and deposit statements.
- `GET /my/documents/<id>/download` — audited signed-URL download. Authorizes
  documents owned by the lease directly, or by the resident's own maintenance
  tickets / inspections.
- `GET /my/inspections` — read-only move-in/move-out inspection reports.
- `GET /my/deposit` — the deposit picture: amount, held-in-trust status, and
  the disposition (deductions, refund, statement) after move-out.

### Maintenance requests (`/account/maintenance`)

Mounted by the `maintenance` module:

- `GET /my/tickets` / `POST /my/tickets` — the resident's requests on their
  lease; creating one validates category/priority, stamps the reporter,
  captures the **location** (where in the home), **access notes**, and
  **permission-to-enter**, emits the vendor webhook, and notifies
  maintenance staff (`maintenance_request` template).
- `GET /my/tickets/<id>` — the request plus its resident-visible timeline
  (public replies with author names + status changes; staff-only internal
  notes are filtered out) and attachments. A staff **public reply** emails
  the resident (`maintenance_reply` template).
- `POST /my/tickets/<id>/comments` — resident comment (staff notified).
- `POST /my/tickets/<id>/photos` — two-step signed-URL upload of a photo
  against the request (`owner_type = maintenance_ticket`).
- Staff status changes on a resident-reported ticket email the resident
  (`maintenance_update` template), so the loop round-trips.

### Message the manager (`/account/messages` ↔ `/console/messages`)

A new pluggable **`messaging`** module (on by default): one `message_thread`
per conversation on a lease, a flat `message` timeline underneath.

- Resident: `GET/POST /my/messages`, `GET /my/messages/<id>`,
  `POST /my/messages/<id>` (reply; reopens a closed thread). Every resident
  message notifies staff holding `message:read` (`resident_message`
  template).
- Staff: `GET /messages?status=` · `GET /messages/<id>` (`message:read`),
  `POST /messages/<id>/reply` · `PATCH /messages/<id>` (close/reopen)
  (`message:manage`). A staff reply notifies the resident in-app and by email
  (`manager_message` template).
- New permissions `message:read` / `message:manage` — granted to the
  workspace owner, property managers, and back-office; landlords read.

## Move-in / move-out lifecycle

Mounted by the `rentals` module; staff-side reads gate on `lease:read`,
writes on `lease:manage`.

### Inspections

`inspection` (kind `move_in` | `move_out`, status `draft` → `completed`) +
`inspection_item` checklist rows (condition `unrated` | `good` | `fair` |
`poor` | `damaged`). Creating an inspection pre-populates a standard
21-point checklist (pass `blank: true` to start empty); photos ride the
document service (`owner_type = "inspection"`). Completing freezes the
report. Residents see their reports read-only in the portal.

- `POST /leases/<id>/inspections` · `GET /leases/<id>/inspections`
- `GET/PATCH /inspections/<id>` · `POST /inspections/<id>/complete`
- `POST /inspections/<id>/items` · `PATCH/DELETE /inspection-items/<id>`

### Security-deposit disposition

The loop from "tenant moved out" to "deposit settled", riding the Phase 3
trust ledger and the provider payout rail:

```
draft ── finalize ──> processing ──> closed
  ↑                       │
  └────── retry ────── failed
```

- `GET /leases/<id>/deposit` — deposit status + disposition (`lease:read`).
- `PUT /leases/<id>/deposit/disposition` — create/replace the **draft**:
  itemized deductions + notes, validated against the deposit held
  (`lease:manage`). Requires the deposit to have settled into trust.
- `POST /deposit-dispositions/<id>/finalize` — moves real money, so it gates
  on `payout:manage`:
  1. **Withheld deductions** post once:
     `Dr Security Deposits Held + Dr Operating Bank / Cr Trust Bank + Cr
     Other Fee Income` — one balanced transaction that satisfies the trust
     invariant (escrow falls exactly as the liability does; the withheld
     amount lands in operating cash as recognized income).
  2. **The refund** rides the payments provider's payout rail on the durable
     queue (`deposit_refund` job; sandbox by default, ACH live; the
     `payout.paid`/`payout.failed` webhook matches bills → deposit refunds →
     owner draws).
  3. **Settlement** posts `Dr Security Deposits Held / Cr Trust Bank`,
     generates the **disposition statement PDF** into the document service
     (filed on the lease, category `statement`), emails the resident
     (`deposit_disposition_closed` template), audits, and notifies staff.
     A zero-refund disposition settles immediately; a failed refund keeps
     its reason and can be re-finalized.

Trust reconciliation (`GET /accounting/trust-reconciliation`) stays at zero
through the whole flow — the disposition never moves escrow cash except
against the deposit liability.

## Console

- **Messages** (`/console/messages`) — thread list with status filter and
  awaiting-reply hint, inline conversation view, reply/close/reopen.
- **Lease detail** (`/console/leases/{id}`) gains an **Inspections** card
  (create move-in/move-out, rate the checklist, attach photos, complete) and
  a **Security deposit** card (trust status, deduction draft builder,
  finalize-and-refund with live status).

## Audit & templates

New audit actions: `message_thread.create/update`, `message.send`,
`inspection.create/update/complete`,
`deposit_disposition.create/update/finalize/settle` (plus the existing
`ticket.*` and `document.*` actions from the portal routes).

New notification templates: `resident_message`, `manager_message`,
`maintenance_request`, `maintenance_update`, `deposit_disposition_closed`.

## Schema

Migration `m20240101_000031_resident_portal`: `message_thread`, `message`,
`inspection`, `inspection_item`, `deposit_disposition`, `deposit_deduction` —
all tenant-scoped, indexed by their parent, with enforced RLS.

## Demo data

The seed gives Taylor Brooks (`taylor@example.com` / `password`) a
resident-reported request in triage, an open conversation with a staff reply,
and a completed move-in inspection — so `/account/lease`,
`/account/maintenance`, and `/account/messages` all light up on first login,
and `/console/messages` has a thread awaiting a reply.
