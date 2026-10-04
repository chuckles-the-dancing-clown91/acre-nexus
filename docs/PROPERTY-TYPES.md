# Property types, units, appliances and meters

A property's type decides how it is rented and what the console offers for it.

| Type | Rented as | Units |
|---|---|---|
| Single-family home, townhome, condo, manufactured home | One home | Exactly one, called **Home**, made automatically |
| Apartments (`multi_family`), commercial | Apartments or suites | As many as the building has |
| Campground, RV park | Sites by the night | None: sites are drawn on the site map |
| Land | Nothing yet | None |

The type is a fixed list (`property_kind.rs`, mirrored in `lib/propertyKind.ts`).
Other spellings are accepted and made canonical ("Apartments" and "multifamily"
become `multi_family`). A property with no type is treated as an apartment
building when it has more than one unit, otherwise as a house.

- A house can't be given a second unit, a building can't become a house while it
  has several, and campground or land properties refuse units. The API explains
  why.
- Migration 082 tidied the stored spellings, gave every existing house its
  **Home** unit and moved its lease onto it.
- **A lease is always on a unit.** For a house it's picked for you. For a
  building you choose one (the application-to-lease conversion too), and it has
  to belong to that property.
- Add a property from **Properties, Onboard**: pick the type first, then the
  address, then the layout (a house's bedrooms and rent, or a building's list of
  units, typed or counted up from a prefix). `POST /properties/onboard` takes
  `unit_list`.

## Units

`GET /properties/<id>/units` lists a property's units with who lives there
(only for people who can read leases), and how many appliances, meters and open
work orders each has. A maintenance crew can see units, since the equipment is
theirs; they can't see tenants. A unit has a number, floor, bedrooms, baths,
square feet, market rent, status and notes.

Each unit has its own page (`/console/properties/<id>/units/<unit>`) with its
**Appliances**, **Meters**, **Utility agreement** and **Work orders**. A
building's own page keeps what serves the whole building (boilers, roof, house
meters).

## Appliances

`/assets` already took a `unit_id`; the property and unit pages now add, edit
and retire equipment (name, type, make, model, serial, installed, warranty,
expected life, where, notes).

## Meters and utilities

A meter (`meter`) is on a property (a house meter) or on a unit: electric, gas,
water, sewer, trash, internet or other, with its number, location, provider,
what it counts in, **who pays** (tenant, landlord, shared) and a billing note.
Readings (`meter_reading`) record the number, the date and why (routine,
move-in, move-out), and may point at a lease. Readings only count up; a swapped
meter is retired and a new one added. Property managers and maintenance crews can
add meters and log readings.

`GET /properties/<id>/utilities?unit_id` works out the **utility terms**: per
utility, who pays, the provider and which meters. A unit's own meters win over
the building's; mixed payers mean shared. The lease's utility agreement is built
from this, so what the lease says is what the meters say.

API: `GET|POST /meters`, `PATCH /meters/<id>`, `GET|POST /meters/<id>/readings`,
`GET /properties/<id>/utilities`.
