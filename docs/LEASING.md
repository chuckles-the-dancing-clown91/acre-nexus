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
3. SCREEN    background_check job → screening_status + screened_at land on the application
                │   auto-approve setting ON + cleared  → Approved automatically (applicant emailed)
                │   otherwise                          → staff notified: "screening finished, review"
                ▼
4. DECIDE    Approved (applicant emailed) │ Declined (applicant emailed) │ Withdrawn
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
   when the lease activates.
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
3. **Screen** — the screening job's completion writes `screening_status` /
   `screened_at` onto the application. With the **`applications.auto_approve`
   setting** on, a cleared check advances the application to `Approved` on the
   spot (automated transition, `actor = None`); otherwise staff get an
   "application screened" notification and decide.
4. **Decide** — `POST /applications/<id>/advance` (or `PATCH`) through the
   validated state machine; approval and decline each email the applicant.
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
funnel: `GET /public/sign/<token>` (first open marks `viewed`),
`POST /public/sign/<token>` (typed name + explicit ESIGN/UETA consent), and
`POST /public/sign/<token>/decline`. The frontend serves the signing page at
`/sign/<token>?tenant=<slug>` (`PUBLIC_APP_URL` builds the link).

Every act lands in `esign_event` — the **ESIGN/UETA audit trail** — with signer,
IP, and user agent. When the last signer signs, completion is automatic: the
lease document is marked signed (all signer names, the pinned hash), the lease
activates and occupancy syncs, a **signed PDF** (document + signature
certificate) is rendered by the in-tree text→PDF writer (`api/src/pdf.rs`) and
stored in the document service as `signed-lease-agreement.pdf` on the lease,
the property's workflow auto-advances to `leased` (when its strategy has that
stage and it hasn't reached it), and signers + staff are notified
(`esign_completed` / `esign_completed_staff`).

Schema (migration `m20240101_000020`): `esign_envelope`, `esign_signer`,
`esign_event` — tenant-scoped with enforced RLS.

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
