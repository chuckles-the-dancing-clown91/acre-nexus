# Calendar / Scheduling / Reminders

The cross-cutting scheduling engine (issue #54): one place for everything
with a due date — lease renewals, license and insurance expirations, tours,
inspections, and custom dates — notified through the Phase 1 substrate at
configurable lead times, and aggregated on one console calendar.

This is substrate, not a one-off feature: renewals (#44), license/insurance
expirations (#45), and tour scheduling all ride the same `reminder` row and
scan job.

## The `reminder` entity

| Field | Meaning |
| --- | --- |
| `subject_type` / `subject_id` | What it's about: `lease` \| `license` \| `insurance` \| `tour` \| `inspection` \| `custom`, optionally pointing at the subject row |
| `due_date` | `YYYY-MM-DD` |
| `lead_days` | Days before the due date to notify, e.g. `[30, 7, 1]` (0 = the day itself) |
| `recipients` | External email addresses; staff holding `calendar:read` are always notified in-app/push |
| `fired` | Lead times that have already fired — a reminder never double-sends |
| `status` | `active` \| `done` \| `cancelled` |

## The scan (the `billing_cycle` pattern)

One durable, self-rescheduling `reminder_scan` job per tenant (ensured at
boot and at tenant provisioning, owned by the `calendar` module) runs on the
configured interval and, idempotently:

1. **syncs lease renewals** — every active lease with an `end_date` keeps
   one active `lease` reminder ("Lease renewal — {tenant}"), created with
   the workspace's default lead times and re-dated (leads re-armed) if the
   lease end moves. Gated by `calendar.lease_renewal_sync`.
2. **fires due reminders** — for each active reminder, every lead time
   whose window has opened (`days_left <= lead`, not yet in `fired`)
   notifies **once**: the `reminder_due` template fans out to staff holding
   `calendar:read` (in-app + push + chat via `notify_staff`) and each
   external recipient gets an `auto_email`. A reminder created late fires
   once for the most urgent open lead; all opened leads are marked fired.
   The notification layer's idempotency key
   (`channel:reminder_due:reminder:<id>:lead_<n>`) backstops the `fired`
   list, and reminders more than 30 days overdue stop nagging (the console
   still shows them overdue). Every firing audits as `reminder.fire`.

## API

| Method | Path | Permission |
| --- | --- | --- |
| GET | `/reminders?from=&to=&subject_type=&status=` | `calendar:read` |
| POST | `/reminders` | `calendar:manage` |
| PATCH | `/reminders/<id>` (edit, `done`, `cancelled`; re-dating re-arms leads) | `calendar:manage` |
| DELETE | `/reminders/<id>` | `calendar:manage` |

Console: **`/console/calendar`** — a month grid with per-day reminder chips
colored by subject type, an upcoming list with done/cancel actions, and a
create dialog. `property_manager`, `back_office`, and `leasing_agent`
manage; `maintenance` and `landlord` read.

## Workspace settings

| Key | Default | Meaning |
| --- | --- | --- |
| `calendar.default_lead_days` | `"30,7,1"` | Lead times for new reminders (comma-separated days) |
| `calendar.scan_interval_secs` | `3600` | How often the scan runs |
| `calendar.lease_renewal_sync` | `true` | Auto-maintain a renewal reminder per active lease |

## Definition of Done (how to see it work)

Create a `license` reminder and a `tour` reminder due next week, and
activate a lease with an end date: the scan (within the hour, or on boot)
creates the renewal reminder, and each reminder fires `reminder_due` at its
configured lead times — visible in the in-app inbox, the notification log,
and the audit trail — with all three on one console calendar.
