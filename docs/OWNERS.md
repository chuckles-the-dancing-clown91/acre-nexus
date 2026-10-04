# Owners: approvals, sign-off, statements, and the owner portal

Where a property's owner has a say, and where they see their money.

## Who the owner is

A property belongs to an LLC (`property.llc_id`); the LLC's biggest stake in
`entity_ownership` names the **owner** (the CRM `owner` record). That owner's
email and phone are where asks go. An owner can hold a login: **Invite** on
the owner (`POST /crm/owners/<id>/invite`, `entity:manage` + `member:manage`)
creates or links a user, gives them the `landlord` persona assigned to their
LLCs, sends the set-password link, and stores `owner.user_id`. In the app, an
account whose only memberships are `landlord` lands on the owner portal.

## Approval before the work

Setting **Owner approval over** (`maintenance.owner_approval_cents`, default
$500; 0 never asks). An owner can have their own limit
(`PATCH /crm/owners/<id>/approvals` `{approval_limit_cents}` or
`{clear_limit: true}`).

- Sending tasks to a vendor on a work order whose **estimate** (tasks plus
  parts, `GET /tickets/<id>/costs`) is at or over the owner's limit is
  refused with a 409 that says so, unless an `approval` covers it
  (`approved` or `overridden`) or the request carries
  `approval_override_reason`, which goes ahead and records an `overridden`
  ask and the audit event `owner_approval.override`.
- Staff **ask the owner** from the work order (`POST /tickets/<id>/approvals`
  `{amount_cents?, note?}`; the Owner panel, or the prompt on the refused
  send). The ask is emailed and texted (`owner_approval_request`) with a link
  `/approve/<token>`; the work order goes `on_hold`, waiting on `owner`, with
  a follow-up in three days.
- The owner **approves or declines** from the link (`GET|POST
  /public/approve/<token>` `{approve, note?}`) or the portal. Approval reopens
  the work order; a decline leaves it on hold waiting on `other`. Staff hear
  either way (`owner_approval_decided`), and can record an answer given by
  phone (`POST /approvals/<id>/decide`).
- Pending asks are **nudged** every two days, three times at most, by the
  helpdesk scan (`owner_approval_reminder`).

`GET /tickets/<id>/approvals` is the staff view: the owner, their limit, the
estimate, `needs_approval`, and every ask with its answer.

## Sign-off after the work

Setting **Owner sign-off on finished work** (`maintenance.owner_signoff`,
default on). When a work order on an owned property goes to `resolved` with
a cost on it (lines, expenses, approved quotes), a `signoff` ask goes out
(`owner_signoff_request`) with the cost, what was done, the public updates,
and the photos. Signing off closes the work order; a dispute puts it back on
hold waiting on the owner with the owner's note, and staff hear.

## The owner portal (`/account/owner`)

All under `/my/owner/*` (no permission; the signed-in user must be an owner
by `owner.user_id` or email):

- `GET /my/owner`: holdings (LLCs, properties with occupancy, rent and open
  work), what's waiting on them, open work with each ticket's ask, the month
  so far, and their limit.
- `GET /my/owner/work?all=`, `GET /my/owner/approvals`,
  `POST /my/owner/approvals/<id>` `{approve, note?}`.
- `GET /my/owner/statement?month=YYYY-MM` and `/my/owner/statement.pdf`: per
  LLC, rent collected, expenses by account, management fee and net (from the
  owner statement report), plus the work done that month with its cost and
  the asks answered.

## Monthly statement email

Setting **Monthly statement day** (`owners.statement_day`, default 3; 0 off).
On and after that day each month, the helpdesk scan emails every owner with
an email and holdings last month's statement once (`owner_statement`, deduped
through the notice log), linking to the portal page.

## Settings, templates, schema

- Settings group **Owners**: the three above.
- Templates: `owner_approval_request`, `owner_approval_reminder`,
  `owner_signoff_request`, `owner_approval_decided` (staff),
  `owner_statement`.
- Migration `m20240101_000072`: `owner.user_id`, `owner.approval_limit_cents`,
  table `owner_approval` (kind, amount, status, token hash, notes, who and
  when, override reason, nudges).
