# Vantedge — what's left, and how we build it

Written after phase 2C shipped (properties, appliances, the parts loop, close-out,
the Alpha link). It covers the next eight areas the business asked for. Each has
what exists today (checked against the code, not remembered), what's missing, the
design, and the order. Legend: ☐ planned · ◐ in progress · ☑ shipped.

Order of work, and why: the **audit trail** goes first because every other area
writes to it; **business profile** and **turnover** are small, high-value and
independent; the **issue catalog** completes the ticket story; **site maps** are the
biggest build and the new product surface; **home search + onboarding** ride on the
map and on the autofill work already done.

| # | Area | Status |
|---|------|--------|
| 1 | Audit trail: who changed what on which property | ☑ |
| 2 | Business profile, Google reviews and integrations (client + Vantedge staff) | ☐ |
| 3 | Turnover with step logic | ☐ |
| 4 | Issue catalog → generate ticket → parts → shopping list | ☐ |
| 5 | Site maps: apartment layouts and campgrounds | ☐ |
| 6 | Tenant home search | ☐ |
| 7 | House onboarding with autofill | ☐ |
| 8 | Campground reservations (follows the map) | ☐ |

---

## 1. Audit trail: who changed what on which property ☑

**Today.** `audit_log` has two layers: a fairing that logs every request, and ~250
domain events written with `audit::record`. `GET /admin/audit` returns the latest
500 rows, filterable by action only. There is **no per-property history, no
before/after values** (`property.update` is recorded with no metadata; a setting
edit stores only the new value), no pagination, no date filter, no export, and no
retention rule. A Vantedge employee acting on a customer's workspace is flagged
`impersonated` in the request scope but that flag never reaches the log.

**Shipped.** The design below, plus a fix found on the way: `GET /admin/audit`
did not filter by workspace (the table sits outside row-level security), so a
workspace-bound principal could have read other workspaces' rows — it now
filters, and a regression test covers it. Tenant owners now hold `audit:read`.
Contact details (email, phone) are masked in diffs so personal data isn't
copied into the trail. Property, unit, lease, listing, work order and appliance
edits and creations are covered; later areas write to it as they land.

**Design.**
- `audit_log` gains `property_id` (indexed) and `support` (true when a Vantedge
  employee made the change while acting on a workspace) and `support_user_id`.
- A `audit::change(...)` writer takes the entity's **before and after** (both are
  serializable models), computes a field diff, drops noise (timestamps, hashes),
  masks secrets, and stores `{changes:[{field, from, to}], summary}`.
- Wired into the writes that matter: property, unit, asset, listing, lease, ticket
  (status/assignee/cost), setting, business profile, site map, turnover steps.
- `GET /properties/{id}/history` (everything that touched the property, its
  units, assets, tickets and listings), `GET /audit` with `property_id`,
  `target_type`, `target_id`, `actor`, `from`, `to`, `support` and a cursor, and
  `GET /audit/export.csv`.
- UI: a **History** tab on the property (who, when, a plain sentence, the
  before→after, a "Vantedge support" badge), and a filterable audit page.
- Retention: a `audit.retention_days` setting (default: keep) with a nightly job.

## 2. Business profile, Google reviews and integrations ☐

**Today.** Alpha has the whole thing: business name/phone/email/address/website,
a Google Business Profile search-and-pick, a review link, review asks after a
finished visit (never gated, no incentives), Google reviews fetched through
Places (New) and cached (only the place id is stored, per Google's terms), and a
Connections page where keys are write-only. **Vantedge has none of it** — no
business profile, no hours, no social links, no review fields. Platform staff can
already impersonate a workspace (`POST /platform/impersonate`, with a reason, a
30-minute session and an audit row) and reach `PUT /settings/{key}`.

**Design.**
- `business_profile` (one per workspace): name, legal name, phone, email, address,
  website, **hours** (per day, with closed days and holiday notes), Google place id,
  review URL, Facebook / Instagram / Yelp / Nextdoor / Google Business links,
  tagline, service area, license number.
- **Reviews** (port of Alpha's): settings (show on site, minimum stars, ask after
  a resolved work order / signed lease / move-in, channel, timing, "ask again
  after N months", English and Spanish message), a request log with click
  tracking, Places search → pick → cached reviews, falls back to testimonials.
- **Connections** card per integration (Google, Stripe, Twilio, Checkr, Gusto,
  maps) with set / test / remove; secrets never come back out.
- **Staff on a call**: a "Support" mode on the platform console — pick the
  workspace, state the reason, and the same Settings screens open with a banner;
  every edit is stamped `support` in the audit trail (area 1) and visible to the
  customer in their own history.

## 3. Turnover with step logic ☐

**Today.** Completing a move-out inspection opens one make-ready ticket and sets
the unit to `make_ready`. That ticket is the whole turn. There is **no checklist,
no sequence, no vendor order, no target date, and no "ready to lease" gate**.

**Design.**
- `turn_template` → ordered `turn_template_step` rows (title, who — office /
  maintenance / vendor / leasing, **depends on** other steps, due offset from
  move-out, required photo, required before "ready"). Defaults ship in the seed:
  notice & forwarding address → move-out inspection → deposit disposition → trash
  out → repairs → paint → flooring → deep clean → rekey → pest/HVAC service →
  final inspection → photos → list.
- `turn` per move-out (unit, lease, target ready date, status) with `turn_step`
  rows copied from the template, each with an owner, due date, status
  (`blocked → ready → doing → done / skipped`), notes, photos and the ticket it
  spawned. A step becomes **ready only when its dependencies are done**.
- **Gates**: the unit can't go `available` and the listing can't publish until
  every required step is done; the office can override with a reason (audited).
- Steps can open work orders (so parts, close-out and costing all apply) and post
  their cost to the turn; the turn shows **days vacant and cost to turn**.
- UI: a turn board per property and a "Turns" page: progress bars, blocked steps,
  who is holding things up, the days-vacant clock.

## 4. Issue catalog → generate ticket → parts → shopping list ☐

**Today.** A resident or employee opens a ticket with a free-text title and one of
six hard-coded categories. There is **no catalog of common issues** and nothing
links an issue to default priority, steps or parts. (The parts loop, close-out and
stock all exist and work once a ticket has parts.)

**Design.**
- `issue_type`: name ("Leaking faucet"), category, default priority, how long it
  usually takes, steps (a short checklist), **default parts** (stock item +
  quantity, or a typed part), safety flags (water/gas/electrical → urgent), and
  whether it usually needs access permission. A seed of ~40 common issues.
- Employee flow on a unit: **choose the issue → Generate ticket** — one call opens
  the ticket with the right category/priority/SLA, the steps, and the parts
  pre-listed as *might need*; stocked parts are reserved, the rest land on the
  shopping list for the night-before close-out.
- Resident portal: pick from the same catalog ("What's wrong?") with photos; the
  office sees the suggested parts before dispatch.
- Learning loop: the parts actually *used* on resolved tickets of an issue type
  adjust that type's suggested parts (shown as "usually also needs…").

## 5. Site maps: apartment layouts and campgrounds ☐

**Today.** Nothing: there is **no building/floor/site/amenity entity, no geometry
field beyond one lat/lng point, no campsite concept, and no map library** in the
frontend. (Alpha has a mature one: MapLibre GL + Terra Draw for drawing, OpenFreeMap
streets, USGS NAIP imagery, building footprints from OpenStreetMap through
Overpass.)

**Format research (what we chose and why).**
- **GeoJSON, WGS84 (lng/lat), is the storage format.** It is what MapLibre and
  Terra Draw read and write, what OSM tooling imports, and it is a plain JSON
  column. One format covers a real satellite view of a park *and* a drawn plan.
- **Three base layers**, one geometry format:
  1. *Satellite / streets* — a real property: draw over OpenFreeMap + NAIP.
  2. *Plan image* — upload the owner's site plan or floor plan (JPG/PNG/SVG),
     placed with four corner coordinates (MapLibre `image` source); this is how
     campground tools such as [Manage.camp](https://manage.camp/campground-map-software)
     do it — upload a map, then place units and outline areas on it.
  3. *Blank grid* — no basemap, a local metre grid, for a property with no
     usable imagery.
- **Drawing**: [Terra Draw](https://github.com/JamesLMilner/terra-draw) through
  its MapLibre adapter (point, polygon, rectangle, select/edit, snapping).
  [MapLibre vs Leaflet](https://maplibre.org/maplibre-gl-js/docs/plugins/):
  Leaflet is simpler for raster-only maps, but we already ship MapLibre + Terra
  Draw in Alpha and want vector styling and rotation, so Vantedge uses the same.
- **Campsite vocabulary follows OpenStreetMap**: a site is `tourism=camp_pitch`
  inside a `tourism=camp_site` (or `caravan_site`) area, mapped as a point or a
  polygon ([camp_pitch](https://wiki.openstreetmap.org/wiki/Proposed_features/Tag:tourism=camp_pitch),
  [camp_site](https://wiki.openstreetmap.org/wiki/Tag:tourism=camp_site)). Our
  feature kinds and attribute names map 1:1, so a map can be **exported as GeoJSON
  with OSM-style tags** and a park can import its OSM pitches.

**Design.**
- `site_map` (per property: kind apartment / campground / RV park / other, base
  layer, centre, zoom, plan image + corners, published) and `site_feature`
  (kind: building, unit, site, amenity, road, boundary, parking, water, label;
  name; GeoJSON geometry; link to a `unit`; attributes).
- **Apartments**: draw buildings and floors, drop each unit as a polygon linked to
  the unit record, coloured by status (available / occupied / make-ready / down),
  click a unit → its lease, rent, open tickets, turn. Prospects see available
  units on the public map.
- **Campgrounds**: draw each site; attributes — type (tent, RV, cabin, glamping,
  group), length, pull-through, power (30/50 amp), water, sewer, shade, ADA, pets,
  max guests, nightly/weekly/monthly rate; amenities (bath house, dump station,
  laundry, fire ring, playground) as points; roads and boundary as lines.
- Tools: draw/edit/delete, snap, duplicate a site along a row ("add 12 sites"),
  area and length readouts, a legend, GeoJSON import/export, print to PDF.
- Public: published maps appear on the listing page and in search (area 6).

## 6. Tenant home search ☐

**Today.** `GET /public/listings` takes **no filters** and returns every public
listing; the page is a grid. There is no search, sort, map, gallery, saved search,
alert, favourite, compare, tour request or contact form; a prospect's only action
is a full application. `baths` is an integer; a listing has no unit link, pet
policy or amenities. Walk score and schools exist in the backend but aren't shown.

**What renters expect** ([Zumper](https://www.zumper.com/blog/best-online-renter-tools-search-filters-to-find-apartments/)):
price, beds/baths, move-in date, pet policy, amenities, map view, saved searches
with alerts, tours without phone tag, and commute time.

**Design (in this order).**
1. Filters + sort on the API and page: price, beds, baths (halves), move-in date,
   pets, property type, amenities; sort by price/newest/available.
2. Listing detail: gallery, floor plan, neighbourhood (walk score, schools),
   pet and fee policy, "what's included", the site map with the unit highlighted.
3. **Map view** with the same filters (listings carry lat/lng from the property).
4. **Request a tour / ask a question** — a public lead form with preferred times,
   consent and spam protection; creates a lead and a calendar hold.
5. **Saved search + alerts** by email/text on new matches (double opt-in).
6. Favourites and compare (local first, account later).
7. Commute-time filter (a routing provider call, cached).

## 7. House onboarding with autofill ☐

**Today.** A three-step wizard with address autocomplete; the property, loans,
assignments and an enrichment job are created in one call. But enrichment is
**never applied back**: year built, bed/bath/sqft, lot, zoning and flood zone land
in side tables and the property and its units stay as typed. Only the geocode is a
live provider; parcel, tax, valuation, schools and utilities are simulated.

**Design.**
- After enrichment, **propose** values on the property (year built, type, sqft,
  beds/baths, lot, APN) with their source and confidence; accept all or one at a
  time; accepted values are audited (area 1). Auto-accept above a confidence bar
  when the field is empty.
- Create **units** from the answer for multi-family; set `property_type`.
- Real data sources, in order of cost: Census geocoder (live), county ArcGIS
  parcel services (the same adapter Alpha uses), OpenStreetMap building
  footprint and levels (live, free), then a paid parcel provider behind the
  existing sandbox-first switch.
- **A per-house onboarding checklist** with step logic (address verified → data
  filled → photos → units → owner/LLC → financing → insurance → utilities →
  manager assigned → listing), each step owned and dated, using the same step
  engine as turnover (area 3).
- One-click "new property in a new LLC" (already modelled).

## 8. Campground reservations ☐

The map makes sites bookable; this makes them rentable. Availability calendar per
site, stays (check-in/out, nights, guests, vehicle), nightly/weekly/monthly rate
plans with seasons and minimum stays, deposits and balance, add-ons (firewood,
pets), a public "pick your site on the map" booking page, check-in/out board,
turnover steps for a site (area 3), and housekeeping. Depends on area 5.

---

## Cross-cutting rules for all of it

- Every write that changes something a customer would ask "who did that?" about
  goes through the audit writer (area 1), with before/after.
- Vantedge support edits are flagged and visible to the customer.
- New tables are tenant-owned with enforced row-level security, like the rest.
- Anything that calls a paid service is sandbox-first (`LIVE_PROVIDERS`).
- Each area ships with an integration test that walks it end to end, and a
  screenshot review in a browser.
