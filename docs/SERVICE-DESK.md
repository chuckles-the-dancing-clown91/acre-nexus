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

Starter kits, added by name to every workspace that doesn't have them:
shower replacement, toilet replacement, water heater replacement, drywall
patch and paint, unit turn repaint, HVAC seasonal service. Retired kits stay
retired. Kits are seeded server-side in `api/src/servicedesk.rs`; editing kits
in the new console is still to come (the API supports it:
`POST/PUT /issue-templates`).

## On a work order

| | Route |
|---|---|
| Tasks: list, add, edit (status, trade, time, vendor, order), remove | `/tickets/<id>/tasks[/<task_id>]` |
| Send a task to a vendor | `POST /tickets/<id>/tasks/<task_id>/dispatch` |
| Vendors for a trade (matching first, insurance and link status) | `GET /tickets/<id>/vendors?trade=` |
| Estimate vs spent, variance, receipts, trades needing a vendor | `GET /tickets/<id>/costs` |
| Upload a photo, receipt or document | `POST /tickets/<id>/uploads` |
| Files with short-lived links | `GET /tickets/<id>/files` |
| Notes with photos (`document_ids`) | `POST /tickets/<id>/comments` |
| Expenses with receipts | `GET/POST /tickets/<id>/expenses` |

**Vendors.** Any contractor (a counterparty of kind `contractor`) can take
work; their `trades` decide who's suggested. A vendor linked to a partner
system (Alpha Power Wash today, through the partner link) gets the job in
their own board; every other vendor gets the work order by email. The
insurance rule (`compliance.require_coi`) applies to every send.

**Spent** is line items plus expenses plus approved quotes. Expenses can be
marked billable to the owner or reimbursable to whoever paid.

## Schedule

A maintenance plan (`/maintenance-plans`) repeats on a cadence and, when due,
opens a work order. With `issue_template_id` set, that work order starts with
the kit's tasks and parts.

## Who sees what

Property managers, leasing agents and maintenance see and work only their
assigned properties (see `ACCESS-AND-BRANDING.md`). The kit catalog is the
company's; anyone on the desk can use it, but only company-wide roles edit it.
