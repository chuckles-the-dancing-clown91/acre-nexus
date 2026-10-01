# Service desk

Where maintenance work is opened, worked, sent to vendors, and paid for.
Console: `/console/maintenance` (queue), `/console/maintenance/new`,
`/console/maintenance/<id>` (the work order), `/console/maintenance/schedule`
and `/console/maintenance/kits`.

## Job kits

A kit is an entry in the issue catalog (`issue_template`) with:

- **tasks**: title, trade (`plumbing`, `drywall`, `tile`, …), estimated
  minutes, and whether it needs a contractor;
- **parts**: name, quantity, typical unit cost (or a stock item).

Picking a kit on a new work order (`POST /issue-templates/<id>/generate`)
puts its tasks on as line items (`ticket_task`) and its parts on the parts
list, matched to stock where the name matches. `POST /tickets/<id>/kits`
adds a kit to a work order that's already open.

Estimates use two settings: `maintenance.labor_rate_cents` (in-house, default
$75/hr) and `maintenance.contractor_rate_cents` (default $125/hr).

Starter kits are added once per workspace: shower replacement, toilet
replacement, water heater replacement, drywall patch and paint, unit turn
repaint, HVAC seasonal service. After that the catalog is the workspace's;
renamed, changed or retired kits stay that way. Kits are seeded server-side
in `api/src/servicedesk.rs`.

Managers build and change kits at `/console/maintenance/kits/new` and
`/console/maintenance/kits/<id>` (`POST`/`PUT`/`DELETE /issue-templates`):
tasks in order with trade, minutes and a contractor flag, parts with quantity
and typical cost, and running totals. "Copy" starts a new kit from an existing
one. Retiring a kit removes it from the catalog; work orders and routines
already made from it keep their tasks and parts. An older catalog entry with
only a checklist opens with the checklist as tasks.

A maintenance plan (`/maintenance-plans`) repeats on a cadence and, when due,
opens a work order. With `issue_template_id` set, that work order starts with
the kit's tasks and parts.

## Sending a task to a vendor

The send button on a task offers contractors, plus any other counterparty that
lists trades or is linked to a partner system, the ones covering the task's
trade first. The insurance rule applies.

- **Linked to Alpha** (or another partner): the task becomes a job on their
  board, titled "<task> — <work order>", with the work order's details, the
  note, and access notes. Their progress comes back signed: started moves the
  task to doing, finished marks it done and adds their price as a labor line,
  cancelled hands the task back to be sent again. On a work order with tasks,
  the vendor's job moves an untouched work order to scheduled or in progress,
  and resolves it only when every task is done. A work order goes to Alpha
  once; Alpha keys jobs by the work order.
- **Everyone else** gets the task by email.

Checked against a running Alpha: link (ping), send, job created with the right
client, property and notes, then scheduled, started and completed callbacks
back on the work order.

## Who sees what

Property managers, leasing agents and maintenance see and work only their
assigned properties (see `ACCESS-AND-BRANDING.md`). The kit catalog is the
company's; anyone on the desk can use it, but only company-wide roles edit it.
