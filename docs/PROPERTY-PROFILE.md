# Property profile

Everything about one property, at `/console/properties/<id>`, in tabs.

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

Units, open work orders and the people assigned follow.

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

## Schools

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
