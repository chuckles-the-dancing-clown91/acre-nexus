# Property profile

Everything about one property, at `/console/properties/<id>`, laid out like
a listing page: photos on the left, a summary card on the right, then tabs.

## Photos and summary

- **Gallery**: one large photo and four small, "See all N photos" for a
  full-screen viewer (arrow keys, thumbnails), floor plans on their own tab,
  and Add photos for people who can edit. The cover leads.
- **Summary card**: status, type and year built; estimated value (with its
  range) and rent roll (against the market estimate); beds, baths and area;
  gross yield (a year's rent over value), price per square foot and
  occupancy; and **the month**: rent, less the loan payment, property tax
  and insurance (the latest on file, over twelve months), leaving what's
  left before upkeep and management.

## Needs attention, across the portfolio

`/console/attention` (`GET /attention`, `property:read`, narrowed to the
properties in reach) is one list of everything waiting on someone: owner
approvals and sign-offs not yet answered, work orders with no date and no
visit, routines due within their lead time, the per-property suggestions
below for every property, and (for people who see the whole company) vendors
with W-9 or insurance problems. Each line goes where it gets done. Items sort
high first, then by date; the chips at the top filter by kind.

## Getting this house ready

The top of the Overview tab (`components/property/Readiness.tsx`) shows the
house onboarding checklist from `GET /properties/<id>/checklist`
(`property:read`, narrowed to reach). Every step is ticked from the data, in
order, each linking to where it gets done; the first open required step is
marked **next**:

| Step | Done when |
| --- | --- |
| Address placed on the map | the property data has coordinates |
| Property record filled in | the record was fetched and no suggested value is waiting |
| A photo | the property has an image |
| Units set up | units exist, at least as many as the property says it has |
| Owner LLC | an LLC owns it |
| Financing (optional) | a loan is on file |
| Insurance | a policy on file that isn't cancelled or expired |
| Utilities | a utility provider on file |
| A manager assigned | someone is assigned to the property |
| Market rent set | every unit has a market rent |
| Leased or listed | every unit has a current lease, or the property has a public listing |

The same panel lists **suggestions from the property record** with a box to
tick each and **Apply** (`GET /properties/<id>/autofill`,
`POST /properties/<id>/autofill/apply`, `property:write`, audited as
`property.autofill`): property type, **year built** (now read from the
records provider into `property_detail.year_built`, migration 077), and for
a single-unit property the unit's beds, baths and square feet and a
**market rent** from the latest rent estimate. Nothing is written until
someone applies it. When every required step is done and nothing is waiting,
the panel shrinks to one "Ready" line.

## Overview

**To do** comes first: the property's action items and what needs attention.

- **Action items** (`action_item`) hang off the property or anything on it
  (`subject_type`: property, parcel, permit, insurance, school, asset,
  utility, tax, document, unit, plan). Each has a due date, priority,
  assignee, and is open, done or not needed.
  `GET|POST /properties/<id>/action-items`,
  `PATCH|DELETE /properties/<id>/action-items/<item_id>`.
- **Needs attention** (`GET /properties/<id>/attention`) turns what's on file
  into suggestions. "Add" puts one on the list; "Not needed" files it away.
  Either way it isn't suggested again. Suggestions that carry a date (a
  renewal, an inspection) come back next cycle.

| Rule | Priority |
| --- | --- |
| An open permit expired without a final | high |
| An open permit expires within 30 days | high |
| A permit inspection within 14 days | normal |
| An active policy has lapsed | high |
| An active policy renews within 45 days (due two weeks before) | normal |
| No active property policy | high |
| FEMA flood zone A or V with no flood policy | high |
| A warranty ends within 60 days | normal |
| Equipment at or near the end of its expected life | low |
| No schools on file, or a zoned school not confirmed with the district | low |
| No parcel number (APN) | low |

The rules are in `api/src/routes/property_records/attention.rs` and are unit
tested without a database.

Then **About this property**: a description in the team's words, the facts
from the public record (type, year built, beds, baths, living area, lot,
stories, parking, heating, cooling, zoning, parcel), and feature groups
(interior, exterior, construction, utilities, community). An entry may read
`Flooring: Hardwood` to show a label. `PUT /properties/<id>/story` sets the
description and features (a group left out is cleared; blanks and repeats are
dropped); they come back with `GET /properties/<id>/intel`.

Units, open work orders and the people assigned follow.

## History

`GET /properties/<id>/timeline` merges everything that has happened, newest
first: built, acquired and sold, value estimates, tax assessments (dated the
first of the year), leases and listings with rent, loans, deeds, liens,
permits issued and finaled, and policies started. The tab shows the estimate
of value and of rent over time, then the list with filters (sales and value,
rent, loans and title, permits and cover).

## Parcel and money

From public records (`PROPERTY_DATA.md`): APN, county, zoning, subdivision,
legal description, lot and building, heating and cooling, parking, flood
zone, owner of record, last sale, walk score, taxes by year, value and rent
estimate, utilities. Then title (ownership), loans and liens. "Refresh"
re-runs the public-record sources.

## Appliances and systems

The equipment registry (`asset`) for the property, grouped by kind, with age,
life left and warranty. "Replace" or "Service" opens a new work order with
the matching kit already picked (`/console/maintenance/new?property=…&kit=…`):
dishwasher, water heater, thermostat, refrigerator, range, disposal, toilet,
smoke and CO detectors, HVAC, ceiling fan, faucet, blinds.

Below it, **Required by code**: the checks this property owes by law or
common code (see HELPDESK.md, "Required by code"), each on the schedule or
not. "Add all" puts every item that applies on the maintenance schedule as a
routine; conditional ones (a pool, a boiler) are added one at a time. Click a
title for what to do and the rule it comes from.

## Permits and plans

**Permits** (`property_permit`): number, kind (building, electrical,
plumbing, mechanical, roofing, demolition, fence, solar, pool, other),
status with the building department (applied, issued, inspection, finaled,
expired, void), issuing jurisdiction, applied, issued, next inspection,
expiry and final dates, contractor, job value, fee, a linked work order, and
files. Marking a permit finaled with no date finals it today.
`GET|POST /properties/<id>/permits`,
`PUT|DELETE /properties/<id>/permits/<permit_id>`.

**Plans and blueprints** are the property's documents filed as `floorplan`,
`blueprint`, `survey` or `permit` (a permit set). Upload them on the tab;
images and PDFs open in the page.

## Schools and area

The tab opens with the area in four facts: walk score, flood zone, county and
zoning.

Each school row has its own profile: level, grades, district, rating,
distance, address, phone, website, enrollment and notes, plus whether the
address is in its **attendance zone** and when the zone was **confirmed**
with the district. "Zone confirmed" records today.

Rows come from the schools data source (listed as zoned, marked
"estimate") or the team. Editing a row makes it the team's
(`source = manual`), and a data refresh replaces only the source's rows and
skips a level the team has covered.
`GET|POST /properties/<id>/schools`,
`PUT|DELETE /properties/<id>/schools/<school_id>`.

## Insurance

Policies (`insurance_policy`): carrier, kind (property, liability, flood,
earthquake, umbrella, builder's risk, rent loss, other), policy number,
status, term, coverage, deductible, annual premium, agent and files. The tab
totals the premiums and shows the flood zone.
`GET|POST /properties/<id>/insurance`,
`PUT|DELETE /properties/<id>/insurance/<policy_id>`.

## Who can do what

Reading takes `property:read`; changing takes `property:write`. Every route
is under `/properties/<id>/…`, so people scoped to their assigned properties
reach only those; a record on another property answers 404. Files attached
to a permit or policy must be that property's documents.
