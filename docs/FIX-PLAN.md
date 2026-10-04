# Fix plan for the round 2 gaps

The concrete fixes behind `ROADMAP-NEXT.md` areas 9–17, written 2026-10-01 from a
check of the code. Each fix says what is wrong, what changes, where, how it is
tested, and its size (S under a day, M a few days, L a week or more). Paths are
relative to `backend/crates/api/src` unless they start with `frontend/`.

Legend: ☐ planned · ◐ in progress · ☑ done. Batches are the build order.

**Batch A shipped** (F1–F10): `resident_reminders.rs`, `notices.rs`,
`paging.rs`, `routes/jobs`, migration `000059_notice_log`, the Reminders settings
group, and Settings → Schedule in the console. Covered by
`batch_a_limits_jobs_and_reminders` in the integration suite.

## Batch A — Go-live safety and reminders (areas 9 and 10)

| ID | Fix | Size | Status |
|----|-----|------|--------|
| F1 | Limits on lists that can grow | S | ☑ |
| F2 | Job history and run-now (Settings → Schedule) | M | ☑ |
| F3 | Notice log so every automatic message sends once | S | ☑ |
| F4 | Rent due and rent past-due notices | M | ☑ |
| F5 | Autopay failed notice with a pay-now link | S | ☑ |
| F6 | Lease expiry at 90/60/30 days opens a draft renewal | M | ☑ |
| F7 | Inspection appointment reminders and a calendar file | M | ☑ |
| F8 | Warranty and plan-due reminders to staff | S | ☑ |
| F9 | Manager morning digest | M | ☑ |
| F10 | Correct the docs on which jobs run on their own | S | ☑ |

- **F1. Limits.** `GET /applications`, `GET /my/applications` and
  `GET /public/listings` loaded every row. Shipped: `limit` (staff default 200,
  max 500; portal default 100, max 200) and a `before` cursor on the staff list,
  newest first; public search answers at most 200 homes. The response shape is
  unchanged, so existing screens keep working. Files: `routes/applications/list.rs`,
  `routes/applications/portal.rs`, `routes/public/listings.rs`. Test: integration
  test asks for `limit=1` and follows the cursor.
- **F2. Job history.** No screen shows background jobs. Add `GET /admin/jobs`
  (this workspace's jobs: kind, status, run at, attempts, last error, result,
  filterable by kind and status, paged) and `POST /admin/jobs/<id>/run-now` for a
  pending recurring job, both behind `tenant:manage`, plus a per-kind summary (last
  run, last outcome, next run). Console page Settings → Schedule. Test: the
  workspace sees its own jobs and not another's; run-now moves `run_at`.
- **F3. Notice log.** New table `notice_log` (tenant, key, sent at) with a unique
  key per tenant, such as `rent_due:<lease>:<due date>`. Every automatic notice
  claims its key first and skips when it exists, so a job that runs every few hours
  never sends twice. Test: claiming the same key twice returns false.
- **F4. Rent notices.** Today the only rent email is `late_fee_applied`. A new
  `resident_reminders` job per workspace (daily) sends `rent_due` N days before the
  rent day (setting `reminders.rent_due_days`, default 3; skipped for autopay
  leases) and `rent_past_due` the day after the due date while the receivable is
  still open, before any fee. Amount from the same calculation billing uses.
  Files: new `resident_reminders.rs`, templates in `notify/mod.rs`, settings.
  Test: a lease due in three days gets one notice, a second run sends nothing.
- **F5. Autopay failed.** `run_autopay` sends the generic `payment_failed`. Use a
  new `autopay_failed` template with the amount, the reason and the portal pay
  link. File: `payments.rs`. Test: a declined autopay queues `autopay_failed`.
- **F6. Lease expiry.** Staff get a reminder at 30/7/1 days and renewals start by
  hand. At 90, 60 and 30 days (setting `reminders.lease_expiry_days`) staff are
  told, and at the first of those the job creates a **draft** renewal with the
  current rent for a manager to review and send. Nothing reaches the resident from
  this step. Files: `resident_reminders.rs`, `renewals.rs`. Test: a lease ending
  in 85 days gets exactly one draft renewal and one staff notice.
- **F7. Inspections.** Inspections created no reminder. Shipped: the resident gets
  `inspection_reminder` two days and one day before `scheduled_date`, with a signed
  calendar link `GET /public/inspections/<id>/calendar.ics?sig=` that works from
  the email without signing in (a guessed link answers 404), and staff get
  `GET /inspections/<id>/calendar.ics`. The ICS writer is a pure function with
  escaping and line folding. Test: the ICS has one VEVENT with the right date.
- **F8. Warranty and plans.** Warranties are stored but nothing warns before they
  end. Staff hear 30 days before a warranty expires (setting
  `reminders.warranty_days`). Test: an asset whose warranty ends in 20 days
  notifies once.
- **F9. Digest.** No morning summary exists. A daily `manager_digest` email to
  staff with `property:read`: rent late, leases ending in 30 days, tickets past
  SLA, turns past target, tour requests in the last day. Skipped when every list is
  empty; on/off setting. Test: the digest counts match seeded data; empty → no send.
- **F10. Docs.** The roadmap says only two jobs run on their own. In fact six do:
  `billing_cycle`, `reminder_scan`, `helpdesk_scan`, `workforce_scan`,
  `property_photo_scan` and `platform_billing`. None sends resident reminders.

## Batch B — Vendors and money (area 11)

| ID | Fix | Size | Status |
|----|-----|------|--------|
| F11 | Vendor W-9: TIN (encrypted), classification, the 1099 reads it | M | ☑ |
| F12 | Vendor COI with expiry, requests before it lapses, dispatch warning | M | ☑ |
| F13 | Vendor portal: invite, assigned work orders, status, photos, bills | L | ☑ |

- **F11.** `tax_1099.rs` exports `tin: None`. Add W-9 fields to the counterparty
  (legal name, TIN encrypted with the PII key and shown as last four,
  classification, signed date), a staff form, and make the 1099-NEC export use
  them and list vendors paid $600 or more with no TIN.
- **F12.** A `vendor_insurance` record (carrier, policy, general liability and
  workers' comp limits, expiry, document). The resident-reminders job asks the
  vendor 30 and 7 days before expiry. Dispatching a work order to a vendor with no
  current COI needs an override reason (audited).
- **Shipped (F11, F12).** Migration 060 adds `vendor_tax_profile` and
  `vendor_insurance`. Rules live in `vendor_compliance.rs`, routes in
  `routes/vendors`: `GET /entities/<id>/compliance`, `PUT /entities/<id>/w9`,
  `POST /entities/<id>/insurance`, `DELETE /vendor-insurance/<id>` and
  `GET /compliance/vendors`. The TIN is checked for SSN and EIN shape, sealed
  with the PII key, and only the last four come back; the audit trail never holds
  it. The 1099 screen shows the last four and the W-9 legal name and counts who is
  missing a W-9; the CSV or PDF export carries the full number and each download
  is audited (`report.1099_export`). The reminders job emails the vendor 30 and 7
  days before a policy ends and tells staff, once each, skipping a policy already
  replaced. The dispatch gate is the setting `compliance.require_coi` (off by
  default so existing flows keep working): with it on, assigning, creating or
  dispatching a work order to a vendor with no current general liability cover
  answers 409 until a reason is given, and the reason is audited
  (`vendor.coi_override`). The console has a Tax and insurance card on contractor
  pages and asks for the reason when the gate stops a dispatch.
- **F13.** `/vendor` portal on the existing invite and login: the vendor's work
  orders, accept, schedule, on the way, done with photos, and a bill into accounts
  payable. Alpha vendors keep the partner link.

## Batch C — Residents and texts (area 13)

| ID | Fix | Size | Status |
|----|-----|------|--------|
| F14 | Ask for a rating when a work order resolves; accept 1–5 by text | M | ☑ |
| F15 | Text to work order link | M | ☑ |
| F16 | Saved replies, assign a thread, file MMS photos | M | ☑ |
| F17 | Quiet hours and marketing consent | S | ☑ |

- **F14.** Resolving a ticket sends "How did we do? Reply 1–5" by text (or the
  portal link by email). `texts.rs` `record_inbound` matches a lone digit from that
  number within 7 days of the ask and stores it as the ticket review.
- **F15.** An inbound text that reads like a repair (issue-catalog keywords) gets a
  reply with a link to a prefilled request.
- **F16.** Saved replies per workspace, thread assignee, and MMS media downloaded
  into documents on the resident and, when linked, the ticket.
- **F17.** Non-urgent texts wait until 8 AM local; marketing texts need separate
  consent.
- **Shipped (F14–F17).** The rules live in `text_auto.rs`, called from
  `texts::record_inbound` and the notification job. Resolving a resident's work
  order texts "How did we do? Reply 1–5" (or emails a rating link when there's no
  number), once per ticket via the notice log. A reply of 1–5 ("4", "5 stars",
  "5/5") from that lease's number within 7 days of the ask becomes the review,
  is audited, tells staff, and thanks the resident; a stranger's digit is just a
  text. A resident's text with a repair word and a trouble word ("faucet is
  leaking") gets a link to `/account/maintenance?new=1&…` with the title,
  category and description filled in, at most once a day per thread; a catalog
  issue whose words all match names the request. Migration 061 adds the thread
  assignee, marketing consent, filed media on messages and `text_saved_reply`.
  `PATCH /texts/<id>` takes `assignee` (teammates only, the new owner hears in
  the app) and `marketing_consent` (not while stopped); `GET /texts?mine=true`;
  `GET/POST /texts/replies`, `PATCH/DELETE /texts/replies/<id>`. Twilio media
  URLs are fetched with the account's credentials by an `sms_media` job (only
  from `api.twilio.com`, 10 MB cap) and filed on the resident's lease. Quiet
  hours (settings group Texts: on, 21 to 8, `America/Los_Angeles`) hold
  automatic texts until morning; typed replies, password and invite messages,
  and answers to someone who just texted go at once. A text queued with
  `marketing: true` is skipped without consent. Missed-call text-back is still
  open.

## Batch D — Listings, owners, analytics, Spanish (areas 12, 14–16)

| ID | Fix | Size | Status |
|----|-----|------|--------|
| F18 | Listing photos feeding the page, share image and structured data | M | ☑ |
| F19 | Owner portal and spend approvals | L | ☑ |
| F20 | Operations dashboard and portfolio map | L | ☑ |
| F21 | Language on people and Spanish message templates | L | ☑ |
| F22 | Camera barcode fallback (zxing) for browsers without BarcodeDetector | S | ☑ |

Details for these follow `ROADMAP-NEXT.md` areas 12, 14, 15 and 16. F19 is in
`OWNERS.md`; F20 is in `REPORTS.md`.

- **Shipped (F18).** Migration 062 adds `listing_photo` (alt text required,
  caption, position). Photos upload through the documents flow with
  `owner_type = listing`, then `POST /listings/<id>/photos` attaches them (images
  only, this listing's uploads only, up to 30); `PATCH` and `DELETE
  /listing-photos/<id>`, `PUT /listings/<id>/photos/order`. Public listings carry
  `photos` hero first, each with a stable `/public/listing-photos/<id>` address
  that redirects to a 15-minute signed link and stops working once the listing
  is hidden or leased. The listing page shows a gallery with captions, cards show
  the hero, JSON-LD lists the images and the share image puts the hero behind
  the rent. Resized variants are not built yet; images are served as uploaded.
- **Shipped (F22).** The stock page's camera scanner loads `@zxing/browser` on
  demand when the browser has no `BarcodeDetector`. Checked in headless Chromium
  on Linux, which has none, with a fake camera showing an EAN-13.
