# Resident profile and ID card

A resident keeps one profile. It fills their applications, it is their mobile
ID card, and a property manager can read and edit it.

## What is in it

- **Account profile** (`user_profile`): legal name, phone, address, date of
  birth, income, military status, government ID and SSN (write-only).
- **Resident profile** (`resident_profile`, migration 084): employer, emergency
  contact, household members, pets (a list), and prior rentals. Staff notes live
  here too and are never sent to the resident.
- Saving pets keeps the account's `has_pet` and `pet_details` in step, so the
  pet fee rule and applications read the same thing.

## Routes

| Route | Who | What |
|-------|-----|------|
| `GET/PUT /my/resident` | the resident | extras, ID card, "Apply now" prefill |
| `GET /residents/profile?email=` | `lease:read`, within reach | one resident with tenancies and applications |
| `PUT /residents/profile` | `lease:manage`, within reach | edit contact, extras, staff notes |

Staff can only edit a person who has an account in their company; otherwise the
page asks them to invite the person first. Property-limited managers see only
residents with a lease or application on a property they reach, and
`/tenant-history` is narrowed the same way.

## Screens

- Resident: `/account/id` (card with QR), `/account/profile` (work, emergency
  contact, household, pets, rental history), `/account/apply` ("Apply now").
- Console: `/console/residents?email=` from the Tenants page.

## Sample logins

`priya.nair@example.com` (Hearthside, dog and prior rental, full profile),
`taylor@example.com` (Northwind). Password `password`.

## The lease is written from the profile

`POST /leases/<id>/document/generate` builds the lease from the lease, the
property and unit, the resident's profile and the home's meters and equipment
(`leasedoc::build`). The result is stored two ways: the signable text (`body`,
what gets hashed and signed) and `sections` (articles and addenda) for layout.

Articles: parties and premises, term, rent and charges, utilities and services,
appliances and equipment, occupants and emergency contact, additional terms
(from fees), pets, vehicles, late payments, privacy.

Addenda, each on its own page with initials lines:

- **Utility Agreement** when the home has meters: who pays for each service,
  which meter, and the rules for tenant-paid and landlord-paid services.
- **Pet Addendum** when the resident has pets, listing each one.
- **Lead-Based Paint Disclosure** for homes built before 1978.
- Your own addenda from the branding templates, `legal_templates.addenda`:
  `[{ "title", "body", "when": "always" | "has_pet" | "has_vehicle" | "built_before_1978" }]`.
  `{tenant}`, `{landlord}`, `{rent}` and the other lease placeholders work in
  the body.

The console lease page, the signing page and Print all show the laid-out
agreement. Documents made before this change keep showing their plain text.

## What the resident sees of it

`GET /my/home` returns the lease agreement once the office has sent it (never a
draft), who pays for each utility, and the equipment that comes with the home.
The lease page in the resident portal shows all three; the agreement opens in
the same laid-out view the office uses.
