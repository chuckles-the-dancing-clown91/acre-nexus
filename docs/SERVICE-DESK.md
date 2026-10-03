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

Kits are jobs to do, not symptoms: "Replace dishwasher", "Replace
thermostat", "Run a new circuit", "Snake a drain", "Rekey locks". A symptom
("AC not blowing cold") can have many causes, so a work order without a kit
starts from the reported problem, and the job is added once someone has
looked at it.

The catalog ships 26 kits, each with a stable key (`replace-dishwasher`,
`service-hvac`, …), listed in `api/src/kit_catalog.rs`. New catalog kits
reach every workspace as they're added; a kit the workspace renamed, changed
or retired stays that way. The old symptom starters are retired from the
catalog; work orders and routines made from them keep what they have.

### Where to buy a part

A part on a kit or a work order can carry a product link. The store is read
from the link (Home Depot, Lowe's, Amazon, Walmart, Menards, Ace, Grainger,
SupplyHouse, Ferguson, RepairClinic, AppliancePartsPros, PartSelect, Build.com,
Zoro), and the work order shows "Buy at Home Depot". A part with no link shows
searches at Home Depot, Lowe's and Amazon instead. Links must be `http(s)`.
`PATCH /parts/<id>` with `url` sets or clears one.

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

## Action buttons

The work order has one-press updates (`GET /ticket-actions`,
`POST /tickets/<id>/actions`). Each posts a note on the ticket and moves the
status when the step implies it:

| Button | Note | Resident sees it | Status |
| --- | --- | --- | --- |
| On my way | On my way. | yes | |
| Arrived | Arrived on site. | yes | in progress |
| Diagnosed | Diagnosed: *what you found* (required) | yes | |
| Need access | Couldn't get in. We need access to finish this. | yes | on hold, waiting on the resident |
| Waiting on parts | Waiting on parts. | yes | on hold, waiting on parts |
| Parts are in | Parts are in; back on it. | yes | in progress |
| Waiting on vendor | Waiting on the vendor. | staff only | on hold, waiting on the vendor |
| Needs a return visit | Needs a return visit. | staff only | scheduled, with a follow-up date |
| Work complete | Work complete. | yes | resolved |

Starting, finishing, skipping or reopening a task leaves a staff-only note
("Done: Install the new dishwasher"). Notes from buttons are marked as updates;
replies from the resident are marked as theirs.

## Photos and video

Staff and residents attach photos (up to 25 MB) and videos (up to 100 MB) to
a work order and its notes. Residents see their own files and anything staff
shared in a public note.

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
