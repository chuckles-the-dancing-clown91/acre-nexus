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
