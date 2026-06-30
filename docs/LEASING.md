# Leasing Lifecycle — application → onboarding → lease signing

The end-to-end resident journey, with templated lease documents, conditional fees
and discounts, vehicle profiles, occupancy that reflects reality, and a tenant
history view. Built on the existing rentals domain (units / leases / payments).

## The flow

```
Public application ──▶ screening (bg job) ──▶ Approve ──▶ Convert to lease
   (/public/applications)                      (PATCH)      (POST /applications/<id>/convert-to-lease)
                                                                 │
                                                                 ▼
                          draft lease  ──▶ apply fee schedule ──▶ generate document ──▶ sign
                          (status=upcoming)   (auto charges)        (templated)         (status=active,
                                                                                         occupancy synced)
```

1. **Apply** — `POST /public/applications` captures the applicant plus the
   attributes that drive the rest of the flow: `has_pet` / `pet_details`,
   `is_military`. Vehicles can be attached to the application (`POST /vehicles`
   with `application_id`). A screening job runs as before.
2. **Approve** — `PATCH /applications/<id>` → `Approved`.
3. **Convert** — `POST /applications/<id>/convert-to-lease` creates a **draft**
   lease (`status = upcoming`) from the application: copies identity + attributes,
   re-links any application vehicles to the lease, links `lease.application_id`,
   and **auto-applies the fee schedule** — all in one transaction.
4. **Build the lease** — review/adjust charges (`/leases/<id>/charges`), add
   vehicles, then **generate** the lease document.
5. **Sign** — `POST /leases/<id>/document/sign` records a typed signature, flips
   the lease to `active`, and **syncs occupancy** (unit → `occupied`, property
   `occupied_units` recomputed).

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

Documents are stored (`lease_document`) for re-download + audit; PDF export and
true e-signature are deliberate later milestones (the body is plain text today).

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
