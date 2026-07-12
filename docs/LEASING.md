# Leasing Lifecycle — listing → application → screening → lease → signing

The end-to-end resident journey, designed as **one pipeline**: advertise a
property, take an application through any door, screen it, approve, convert to
a lease with the agreement auto-generated, sign electronically, and watch the
listing, occupancy, and property workflow close out on their own. Built on the
rentals domain (units / leases / payments), the document service, and the
notification substrate.

## The pipeline

```
1. LIST      console: POST /properties/<id>/listings   → public website shows it
                │
2. APPLY     three doors, one pipeline (application.source):
                • public website     POST /public/applications      (anonymous)
                • renter portal      POST /my/applications          (signed-in, linked to the account)
                • back office        POST /applications             (staff intake)
                │   applicant emailed "application received" · staff fan-out (in-app/push/chat)
                ▼
3. SCREEN    background_check job → consumer report ordered (Checkr, FCRA consent from intake)
                │   report stored as screening_report · policy verdict → screening_status + screened_at
                │   auto-approve setting ON + cleared  → Approved automatically (applicant emailed)
                │   otherwise                          → staff notified: "screening finished, review"
                ▼
4. DECIDE    Approved (applicant emailed) │ Declined (applicant emailed; adverse-action
                notice auto-sent + filed when the report was adverse) │ Withdrawn
                ▼
5. CONVERT   POST /applications/<id>/convert-to-lease
                • draft lease (upcoming) + identity/attributes/vehicles copied
                • fee schedule auto-applied · application → Leased (event recorded)
                • listing → Pending · lease agreement AUTO-GENERATED (draft)
                ▼
6. SIGN      e-signature envelope (docs: E-signature envelopes below)
                • signers emailed/texted links → view → sign (ESIGN audit trail)
                • or in person: POST /leases/<id>/document/sign
                ▼
7. CLOSE-OUT automatic on the final signature:
                • lease → active · occupancy synced · signed PDF stored on the lease
                • listing → Leased + unpublished · property workflow → "leased"
                • signers + staff notified
```

Every step is visible: the application's pipeline (`application_event`), the
property's process tracker (`workflow_event`), the envelope's ESIGN trail
(`esign_event`), and the audit log all record who did what, when.

1. **List** — `POST /properties/<id>/listings` advertises a property (address
   from the property, beds/baths/sqft defaulted from enrichment); `GET
   /listings` + `PATCH /listings/<id>` manage price, copy, status
   (`Available`/`New`/`Pending`/`Leased`) and public visibility from the
   console's Listings page. The pipeline retires listings automatically
   (`crate::listing_sync`) — `Pending` on conversion, `Leased` + unpublished
   when the lease activates, and back to `Available` when the resident
   declines the envelope.
2. **Apply** — all three doors run the same `applications::intake`: persist
   (with `source`), audit, staff fan-out, the applicant's confirmation email,
   and the screening job. Portal applications are **white-glove**: the
   account's email is forced, and name, phone, pets, military status, and
   stated income all auto-fill from the person's **profile** (`GET/PUT
   /my/profile`), with their profile vehicles (`/my/vehicles`) snapshotted
   onto the application — the tenant only keeps their profile current and
   applies with a move-in date. Renters track applications at
   `/account/applications` and maintain everything at `/account/profile`;
   staff can correct any of it (pets, income, government ID — encrypted at
   rest) through the IAM profile routes (`PUT /admin/users/<id>/profile`).
   Applications capture the attributes that drive the rest of the flow
   (`has_pet`/`pet_details`, `is_military`, vehicles).
3. **Screen** — Phase 4 made this real (see [`SCREENING.md`](SCREENING.md)):
   the job orders a **consumer report** (credit + criminal + eviction)
   through the Checkr provider — with the applicant's FCRA consent captured
   at intake — stores it as a `screening_report`, evaluates the workspace's
   **screening policy**, and writes `screening_status` / `screened_at` onto
   the application (plus `screening.ordered` / `screening.completed` /
   `application.screened` audit events). The policy is settings-driven:
   `screening.min_credit_score` (0 = no floor) and
   `screening.min_income_rent_ratio` (monthly income vs. the listing's rent,
   0 = off), plus any criminal or eviction records, fail an application —
   the reasons land on the report and in the audit metadata;
   `screening.callback_delay_secs` paces the simulated provider. With the
   **`applications.auto_approve` setting** on, a cleared check advances the
   application to `Approved` on the spot (automated transition,
   `actor = None`); otherwise staff get an "application screened"
   notification and decide.
4. **Decide** — `POST /applications/<id>/advance` (or `PATCH`) through the
   validated state machine; approval and decline each email the applicant.
   Declining an applicant whose report carried adverse information triggers
   the **FCRA adverse-action notice** (auto by default, or one click from the
   console) — generated, filed as a PDF against the application, and emailed.
5. **Convert** — `POST /applications/<id>/convert-to-lease` creates a **draft**
   lease (`status = upcoming`), copies identity + attributes, re-links
   vehicles, **auto-applies the fee schedule**, marks the listing `Pending`,
   records the application's `Leased` event, and **auto-generates the lease
   agreement** (opt out with `generate_document: false`) — ready to send for
   signature in one step.
6. **Sign** — send the e-signature envelope (each signer gets an email/SMS
   link) or capture a typed signature in person.
7. **Close-out** — the final signature activates the lease, syncs occupancy
   (which also flips the property's availability status `Vacant` →
   `Stabilized`), stores the signed PDF, closes the listing (`Leased` +
   unpublished — and the public site never shows `Leased` listings even if
   one is left public by hand), advances the property workflow to `leased`,
   and notifies everyone.

## CRM: leads → tours → application (issue #44)

Before the apply funnel there's a **prospect pipeline**. A `lead` is a leasing
prospect that progresses `new → contacted → toured → applied → closed`. Leads
arrive three ways: the monitored leasing inbox creates/updates one from inbound
email (`crate::mail` routes it here — see [`EMAIL.md`](EMAIL.md)), a public
website enquiry, or **manual entry** at the front desk. The Leads console page
(`leasing` module) is the CRM board.

- `GET /leads?status=` (`application:read`) — the pipeline, most-recently-touched
  first, plus the monitored inbox address that feeds it.
- `POST /leads` (`application:write`) — manually enter a prospect (walk-in,
  phone, referral); `source` ∈ `manual` | `website` | `referral` | `walk_in`.
- `PATCH /leads/<id>` (`application:write`) — work a lead: contact details,
  pipeline status, notes.
- `POST /leads/<id>/tour` (`application:write`) — **schedule a showing**: drops a
  `tour` reminder on the calendar (notified ahead through the substrate — see
  [`CALENDAR.md`](CALENDAR.md)) and nudges a brand-new lead to `contacted`.
- `POST /leads/<id>/convert` (`application:write`) — **convert to an
  application** without leaving the platform: the lead's contact details seed a
  back-office intake (`application.source = crm_lead`) that enters the exact same
  screening pipeline as every other door, and the lead is marked `applied` and
  linked to the new application (`lead.application_id`). A lead converts once.

So a prospect moves lead → toured → applied entirely in-console, and the linked
application then rides the pipeline below. `lead` gains `application_id`
(migration `m20240101_000041`).

## Application workflow (pipeline)

An application's `status` is a stage in a validated state machine
(`crate::app_workflow`), so the applications inbox is a real pipeline with an
auditable history rather than a free-text field:

```
New ──▶ Screening ──▶ Approved ──▶ Leased          (main path)
   ╲        │            │
    ╲       ▼            ▼
     ──▶ Declined / Withdrawn  ◀── (off-ramps; re-openable to Screening)
```

- `GET /applications/workflow/catalog` — the stages, off-ramps, and legal
  transitions (drives the UI's advance buttons).
- `GET /applications/<id>/workflow` — one application's current stage, reached
  stages, allowed next stages, and full transition history.
- `POST /applications/<id>/advance` `{ to_status, note? }` — moves the
  application, validating the transition, recording an immutable
  `application_event`, and (→ `Approved`) enqueuing the welcome email.
  `PATCH /applications/<id>` uses the same validated path.

Every transition is stored in `application_event` (mirrors `workflow_event` for
properties) and audited.

## Workspace settings

Everything tunable in this flow lives in the settings catalog
([TENANCY.md](./TENANCY.md#system-settings-setting) → System settings), editable
from the console's Settings page (`tenant:manage`) and audited as
`setting.update`:

| Key | Default | Controls |
|-----|---------|----------|
| `applications.auto_approve` | `false` | Auto-approve a cleared screening |
| `applications.generate_document_on_convert` | `true` | Draft the lease agreement automatically on conversion (per-call override still wins) |
| `application_reuse.enabled` / `.window_days` | `false` / `30` | Reusable applications (below) |
| `screening.min_credit_score` | `0` (off) | Credit floor for screening to clear |
| `screening.min_income_rent_ratio` | `0` (off) | Monthly income-to-rent multiple for screening to clear |
| `screening.callback_delay_secs` | `6` | Simulated provider callback pace |
| `screening.cra_name` / `.cra_contact` | Checkr, Inc. | The consumer-reporting agency cited on adverse-action notices |
| `screening.auto_adverse_action` | `true` | Auto-send the FCRA notice when declining a flagged applicant |
| `esign.link_expiry_days` | `0` (never) | Signing links die N days after the envelope is sent |
| `esign.max_signers` | `10` | Signer cap per envelope |
| `esign.signed_doc_retention_days` | `0` (forever) | Retention stamped on the stored signed PDF |
| `lease_documents.title` | `Residential Lease Agreement` | Title on generated leases + their envelopes |

## Reusable applications (configurable)

When the **`application_reuse.enabled`** system setting is on (see
[TENANCY.md](./TENANCY.md#system-settings-setting) → System settings), a recent
application can be used for any property in the firm without re-applying — bounded
by `application_reuse.window_days` (default 30):

- **Staff**: `GET /applications/reusable?email=` lists an applicant's recent
  reusable applications; `POST /applications/reuse { source_application_id,
  listing_id? }` clones one (carrying the screening result) so it can be converted
  to any property. From the applications page, "Reuse" duplicates an application
  for another property.
- **Public**: on `POST /public/applications`, if the applicant's email already
  has a recent **approved** application in the window, the new application is
  pre-approved and skips re-screening.

Disabling the setting reverts both paths to normal per-listing screening.

## Conditional fees, discounts & amenities

The landlord configures a **fee schedule** (`/fees`) — a reusable catalog. Each
entry has:

| Field | Meaning |
|---|---|
| `kind` | `fee` \| `discount` \| `rebate` \| `amenity` |
| `amount_cents` | non-negative; the **sign is derived from kind** (discounts/rebates subtract) |
| `recurring` | monthly vs one-time |
| `condition_type` | `manual` \| `always` \| `has_pet` \| `is_military` \| `has_vehicle` |
| `verbiage` | lease-document language, with `{placeholder}` interpolation |

`POST /leases/<id>/apply-fees` (also run automatically at conversion) evaluates
each entry's condition against the lease's attributes and the resident's vehicles,
and creates a `lease_charge` for every match — **idempotent per `code`**, so it
never double-applies. Examples shipped in seed:

- `pet_fee` → applies when `has_pet`; verbiage references `{pet_details}`.
- `military_discount` → a negative recurring charge when `is_military`.
- `garage` (amenity, `manual`) → verbiage references `{vehicles}`, pulling the
  resident's car details into the lease.

Manual line items can also be added directly (`POST /leases/<id>/charges`). The
monthly total = base rent + recurring charges (floored at zero).

## Vehicle profiles

`vehicle` is a tenant-scoped profile (make/model/year/color/plate), optionally
linked to an `application`, a `lease`, and/or a renter `user`. Garage/parking
amenities and the lease document pull these in via the `{vehicles}` placeholder.
CRUD at `/vehicles` (`vehicle:read` / `vehicle:manage`).

## Templated lease documents

`leasedoc::render` turns the tenant's `theme.legal_templates` + the concrete
lease, its charges, the resident's attributes, and their vehicles into a finished
agreement. Interpolation is a small pure `{placeholder}` substitution (no external
templating crate). Supported placeholders: `{landlord}`, `{tenant}`,
`{property_address}`, `{unit}`, `{rent}`, `{deposit}`, `{monthly_total}`,
`{start_date}`, `{end_date}`, `{late_fee}`, `{grace_days}`, `{amount}` (per
charge), `{pet_details}`, `{vehicles}`.

- `POST /leases/<id>/document/generate` — render a new draft.
- `GET /leases/<id>/document` — the latest.
- `POST /leases/<id>/document/sign` — typed signature → `signed`, activates lease.

Signing is **tamper-evident**: it records a SHA-256 hash of the document body
plus the signer's IP and timestamp (`lease_document.signed_hash` / `signed_ip`),
so a signed lease can be proven unchanged. Documents are stored for re-download
and **printed to PDF** from the lease detail page (the browser's Save-as-PDF over
the rendered body — no external dependency). For remote, multi-party signing see
**e-signature envelopes** below; a third-party connector (DocuSign-style)
remains a later option.

## E-signature envelopes

The native **remote-signing** flow (roadmap Phase 2): a generated lease document
is sent as an *envelope* to one or more *signers* — resident, landlord,
guarantor, other — each of whom receives a **tokenized signing link** by email
(and SMS when a mobile is on file) through the notification substrate
([`NOTIFICATIONS.md`](NOTIFICATIONS.md)). Possession of the link is the
credential. The token is stored two ways, never in plaintext: a SHA-256 hash
for lookup, plus an AES-256-GCM seal under `SECRETS_ENC_KEY` (the vault
pattern) so reminders re-send the **same** link — earlier emails keep working.

```
envelope: sent ──→ partially_signed ──→ completed      signer: sent ──→ viewed ──→ signed
            ├──→ declined   └──→ voided                          └──────────└──→ declined
```

Console endpoints (mounted by `lease_builder`):

- `POST /leases/<id>/envelope` (`lease:manage`) — create + send. Signers default
  to the lease's resident + the sending user as landlord; the envelope pins a
  SHA-256 of the document body so every party provably signs the same text.
  Signing links are returned **once** and delivered to each signer.
- `GET /leases/<id>/envelope` (`lease:read`) — signers + the full audit trail.
- `POST /esign/envelopes/<id>/remind` — re-send the original links to pending
  signers (a token is re-minted only if its seal can't be opened, e.g. after a
  key rotation).
- `POST /esign/envelopes/<id>/void` — cancel; pending signers are notified and
  the document returns to `draft`.

Public (tokenized) endpoints, resolved via `X-Tenant`/`?tenant=` like the apply
funnel: `GET /public/sign/<token>` (read-only — link scanners and previews
don't pollute the trail), `POST /public/sign/<token>/viewed` (the page calls
it on the signer's **first interaction**, marking `viewed` with IP + user
agent), `POST /public/sign/<token>` (typed name + explicit ESIGN/UETA
consent), and `POST /public/sign/<token>/decline` — which also puts a listing
parked at `Pending` back to `Available`, since the deal died from the
resident's side (a staff **void** leaves the listing alone). The frontend
serves the signing page at `/sign/<token>?tenant=<slug>` (`PUBLIC_APP_URL`
builds the link).

Every act lands in `esign_event` — the **ESIGN/UETA audit trail** — with signer,
IP, and user agent. When the last signer signs, completion is automatic: the
lease document is marked signed (all signer names, the pinned hash), the lease
activates and occupancy syncs, a **signed PDF** (document + signature
certificate) is rendered by the in-tree text→PDF writer (`api/src/pdf.rs`) and
stored in the document service as `signed-lease-agreement.pdf` on the lease,
the property's workflow auto-advances to `leased` (when its strategy has that
stage and it hasn't reached it), and signers + staff are notified
(`esign_completed` / `esign_completed_staff`). Two racing final signatures
can't wedge the envelope — signing locks the envelope row (`SELECT … FOR
UPDATE`) so completions serialize — and if the PDF store hiccups, completion
still goes through and the store is retried by a deferred `esign_store_pdf`
job. Signing **in person** voids any envelope still out on the document (and
a live envelope refuses to sign a document already signed outside it), so an
emailed link can never overwrite an in-person signature record.

Schema (migration `m20240101_000020`): `esign_envelope`, `esign_signer`,
`esign_event` — tenant-scoped with enforced RLS. An envelope carries a
`purpose` (`lease` by default, or `renewal`) so completion applies the right
side-effects (activate a new tenancy vs. bump an existing one — see Renewals),
and the "latest lease agreement" lookups skip renewal addenda via the same
distinction on `lease_document.purpose`.

## Lease renewals (issue #44)

The **ongoing-tenancy** motion: keep a resident by offering renewed terms
(typically a rent increase + extended end date) rather than turning the unit.
A renewal rides the same Phase 2 document + e-signature substrate as the initial
lease — it just modifies, rather than replaces, the agreement.

```
PROPOSE  POST /leases/<id>/renewals {new_rent_cents, term_months|new_end_date?, new_start_date?, notes?}
            • lease_renewal row (proposed) pins current→new rent + the new term
            • a renewal ADDENDUM (lease_document, purpose=renewal_addendum) is generated
            ▼
SEND     POST /renewals/<id>/send {message?, signers?}
            • esign envelope (purpose=renewal) on the addendum → resident + landlord
            • signing links emailed/texted; renewal → sent
            ▼
SIGN     the tokenized public signing page (same as any envelope) → ESIGN trail
            ▼
APPLY    automatic on the final signature (esign::complete_envelope, renewal branch):
            • lease.rent_cents ← new rent · lease.end_date ← new end · status → active
            • renewal → activated · signed PDF filed on the lease
            • the calendar scan re-dates the lease-renewal reminder to the new end
```

- `GET /leases/<id>/renewals` (`lease:read`) — the renewal history, each with its
  signing envelope (signers + audit trail) so the console tracks progress.
- `POST /leases/<id>/renewals` (`lease:manage`) — propose. `new_start_date`
  defaults to the day after the current term; the end is `new_end_date`, else
  `new_start_date + term_months`, else month-to-month. One in-flight renewal per
  lease. A lease that is `ended`/`expired` can't be renewed (make a new lease).
- `POST /renewals/<id>/send` (`lease:manage`) — send the addendum for signature
  (defaults to the lease's resident + the sending user, like the initial lease).
- `POST /renewals/<id>/cancel` (`lease:manage`) — withdraw an in-flight renewal,
  voiding any open envelope so the signing links die.

The renewal envelope is completely separate from the lease-agreement envelope:
the lease page's e-signature card only ever shows `purpose = lease` envelopes,
and the Renewals card shows the renewal's. The term math (`add_months` clamped to
month-end, effective-date defaulting, rent-change %) lives in `crate::renewals`;
the addendum body renders from `crate::leasedoc::render_renewal_addendum`. Titled
by the `lease_documents.renewal_title` setting. Schema: `lease_renewal` +
`esign_envelope.purpose` + `lease_document.purpose` (migration
`m20240101_000041`).

## Property reflects the tenant

`rentals_occupancy::sync_property_occupancy` runs (best-effort) on lease create,
update, convert, and sign: a unit with an active lease becomes `occupied`, one
without reverts to `vacant` (leaving `make_ready`/`down` alone), and the
property's `occupied_units` is recomputed from active leases. The property profile
links to its **tenant history**.

## Tenant history (landlords + back office)

`GET /tenant-history` and `GET /properties/<id>/tenant-history` aggregate leases
into per-resident rows (grouped by email, falling back to name): the full tenancy
timeline, whether they're a current resident, balances owed, and whether the
tenancy originated from an application. Gated by `lease:read`, which both the
**landlord** and **back-office** roles hold.

## Permissions

New: `fee:read` / `fee:manage` (fee schedule), `vehicle:read` / `vehicle:manage`.
Granted so landlords and back-office staff can configure fees and view history;
property managers and leasing agents get the operational subset; renters can
manage their own vehicles. Lease charges + documents reuse `lease:read` /
`lease:manage`.

## Schema (migration `m20240101_000013`)

`fee_schedule`, `lease_charge`, `vehicle`, `lease_document` (all tenant-scoped +
RLS); `application` and `lease` gain `has_pet` / `pet_details` / `is_military`;
`lease` gains `application_id`. See `backend/crates/entity/src/*` for the models.
