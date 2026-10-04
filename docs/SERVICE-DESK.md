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

## Queues and assignment

- **Who has it.** A work order belongs to a person on the team or a vendor.
  `GET /ticket-techs?property_id=` lists who can take work: people assigned
  to the property first (company-wide roles can work anywhere), then the
  lightest load (open work orders and tasks). `PATCH /tickets/<id>` takes
  `assignee_user_id` to assign and `clear_assignee_user` to take it off; the
  assignee is told in the app and by email.
- **Tasks go to people too.** `PATCH /tickets/<id>/tasks/<task_id>` takes
  `assignee_user_id` (`""` clears). Only people with a live, non-resident
  membership in the workspace can be given work.
- **A person's queue.** `GET /ticket-queue` returns the caller's tasks that
  aren't done, with the work order and property for each: started ones first,
  then urgent, then soonest due. Only properties in their reach.
- **The queue page** has Mine, Open, Unassigned, Urgent, Waiting and Done
  with counts, a Whose work filter (everyone, vendors, a person), the team
  and what each has, task progress on each row, and assigning from the row.
  The work list (`GET /tickets`) carries `assignee_name`, `assignee_kind`
  (`tech` or `vendor`), `tasks_total` and `tasks_done`.

## My day (the tech's phone)

`/console/my-day` is the technician's screen for the day, built for a phone:
the visits booked with them today in time order, then the rest of their
queue, one card per work order with its tasks. Each card has directions, the
resident's number when there's a visit, "Nobody home" and "Visit done" on a
confirmed visit, and a checkbox per task. People on the team (an employee
profile) also get the clock: **Start the clock** on a card clocks them in on
that work order (`POST /me/clock/in` with `kind: work_order`), **Switch
here** moves a running clock to another job, **Stop** clocks out, and the
card shows the minutes logged there today. The bar at the top shows today's
and the week's hours and any missed punches. It composes existing routes:
`GET /appointments?from&to&assignee`, `GET /ticket-queue`, `GET /me/clock`,
`GET /me/time?from&to`.

## Scheduling the visit

A work order's **Visit** panel offers up to four time windows
(`POST /appointments` with `ticket_id`). The resident on the lease gets them
by email and text with a one-time link (`/book/<token>`), and sees them on
the request in the portal. They pick one, or say none work and suggest
another time. Picking moves the work order to **scheduled** with that day as
its due date and a public note ("Scheduled: Tue, Oct 6, 1 PM to 3 PM");
declining tells staff what time was asked for. Staff can confirm a time
agreed on the phone, change it, cancel, and mark the visit done or nobody
home. A new offer on the same work order replaces the open one.

Reminders go to the resident (email and text) and the person going (in-app)
at `appointments.reminder_hours` before (default 24 and 2), once each.
Windows default to `appointments.window_minutes` long (120). Times staff
type are read in the workspace's time zone (`texts.timezone`).

`GET /appointments?from&to&assignee&property_id&status` is the calendar at
`/console/calendar`: a week of visits (repairs, showings, inspections) and
reminders due, filtered to one person.

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

## Sending tasks to a vendor

Tick one or more tasks and send them to one vendor as **one job**
(`POST /tickets/<id>/dispatch-tasks`, or
`POST /tickets/<id>/tasks/<task_id>/dispatch` for a single task): one email
listing the tasks, or one entry on a linked vendor's board titled "3 tasks —
<work order>". A finished or skipped task can't be sent. Each task keeps how
it went (`dispatch_via`: `partner` or `email`) and the note, and the work
order gets a staff note ("Sent to Rose City Appliance (by email): …").

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
- **Everyone else** gets the task by email, with a link to answer from.

### The vendor's link

Every batch sent to a vendor carries one link (`/vendor/<token>`; the
dispatch email and text carry it, and a linked vendor's board gets it in the
job note). It needs no account. From it the vendor sees the work order, the
property, the office's note and access notes, and their tasks, and can:

- **Accept**, optionally saying when they'll come. A time books the visit on
  the calendar (confirmed by `vendor`, the vendor on it), moves the work order
  to scheduled, and tells the resident, the same as a picked window. Their
  tasks move to doing.
- **Decline**, with a reason. The tasks go back to unassigned (no vendor, no
  dispatch), so they can be sent to someone else; staff with
  `maintenance:manage` hear (`vendor_task_declined`).
- **Send photos** (before and after) and **the invoice** as a file. They land
  on the work order's files like any other.
- **Send their invoice**: an amount and what it covers, with the uploaded file
  attached. It lands as a vendor expense on the work order (category repairs,
  billable to the owner, recorded by nobody) for the office to approve.
- **Mark it done**, with a note. Their tasks close, the resident sees the line
  on the request, and staff hear with the invoice total.

Each task keeps the vendor's last answer (`vendor_response`: accepted,
declined or done, with when and what they said), shown on the task list. A
declined or finished batch can't be answered again; sending the tasks again
mints a new link. Public routes: `GET /public/vendor/<token>` and
`POST …/accept | /decline | /done | /uploads | /invoice`.

### Inviting a vendor to Alpha

In the send dialog, an unlinked vendor with an email shows **Invite to
Alpha** (`POST /entities/<id>/alpha-invite`). They get the `alpha_invite`
email with a sign-up link (the `partners.alpha_join_url` setting, by default
Alpha's `/partners/join` page) prefilled with their business, contact, email,
phone and who sent them. Alpha files it as a lead (source `vantedge`) for
Alpha's office to set up and link back. The invite is remembered on the vendor
(`alpha_invited_at`), so the dialog shows "Invited" after.

Checked against a running Alpha: link (ping), send, job created with the right
client, property and notes, then scheduled, started and completed callbacks
back on the work order.

## Who sees what

Property managers, leasing agents and maintenance see and work only their
assigned properties (see `ACCESS-AND-BRANDING.md`). The kit catalog is the
company's; anyone on the desk can use it, but only company-wide roles edit it.
