# Vantedge — what's left, and how we build it

Round 1 (areas 1–8) was written after phase 2C shipped and covered the eight
areas the business asked for. Areas 1–5 are shipped, 6 and 7 are part done, and 8
is still planned. **Round 2** (areas 9–17, further down) was mapped on 2026-10-01
after a check of every open roadmap item against the code. Each area has what
exists today, what's missing, the design and the order. Legend: ☐ planned ·
◐ in progress · ☑ shipped.

Also shipped since round 1 was written, outside the eight areas: Alpha ↔ Vantedge
single sign-on, embeddable website widgets (`docs/SSO-AND-EMBEDS.md`), and a
server-rendered public site for search (`docs/SEO.md`).

| # | Area | Status |
|---|------|--------|
| 1 | Audit trail: who changed what on which property | ☑ |
| 2 | Business profile, Google reviews and integrations (client + Vantedge staff) | ☑ |
| 3 | Turnover with step logic | ☑ |
| 4 | Issue catalog → generate ticket → parts → shopping list | ☑ |
| 5 | Site maps: apartment layouts and campgrounds | ☑ |
| 6 | Tenant home search | ◐ |
| 7 | House onboarding with autofill | ◐ |
| 8 | Campground reservations (follows the map) | ☐ |
| 9 | Go-live hardening: limits, job history, backups, end-to-end tests | ◐ |
| 10 | Reminders that run themselves, for residents and managers | ☑ |
| 11 | Vendor portal and compliance (W-9, COI, 1099) | ☐ |
| 12 | Owner portal and spend approvals | ☐ |
| 13 | Texts, round 2: text to work order, ratings by text, team inbox | ☐ |
| 14 | Listing media, map search, saved searches and listing feeds | ☐ |
| 15 | Operations analytics and the portfolio map | ☐ |
| 16 | Spanish for everything a resident sees | ☐ |
| 17 | Family-plan features: related-party guard, Foundation mode, raw land | ☐ |

The concrete fixes, sized and in build order, are in [`FIX-PLAN.md`](FIX-PLAN.md).

**Order for round 2:** 9 first, because Bree's portfolio can't go live on lists
with no limits, jobs nobody can see, and a restore nobody has tried. Then 10,
since it turns data we already hold into fewer calls to the office. 11 comes
before 12 because the 1099 export is already wrong without vendor TINs, and an
expired COI is a liability today. 13 and 14 grow what residents and prospects
see. 15 needs a few months of turn and ticket history to be worth reading, so it
comes later on purpose. 8 waits for a campground customer. 16 and 17 run
alongside whenever their owner asks.

---

# Round 1

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

## 2. Business profile, Google reviews and integrations ☑

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

## 3. Turnover with step logic ☑

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

## 4. Issue catalog → generate ticket → parts → shopping list ☑

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

## 5. Site maps: apartment layouts and campgrounds ☑

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

## 6. Tenant home search ◐

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

## 7. House onboarding with autofill ◐

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

## Round 1: what shipped, and what is left

- **2 Business profile and Google reviews** ☑. One profile per workspace, Google
  place search and details (Places API New), display rules, a public reviews
  strip. Only the place id is stored. Without a live key, sample data answers and
  is marked. Vantedge support and account-manager roles can edit it; edits are
  flagged as support changes. Left: Facebook, Yelp and Nextdoor review pulls.
- **3 Turnover** ☑. A reusable step engine (templates, runs, dependency-driven
  readiness, required-step gate with audited override, photo-required steps,
  work orders that complete their step, unit-vacant gate, days vacant and cost).
  A completed move-out inspection starts the turn. Left: onboarding and site-turn
  templates on the same engine, due-date reminders.
- **4 Issue catalog** ☑. Starter set per workspace, editable. Generate opens the
  ticket, lists the usual parts tied to stock by name, and builds the shopping
  list. Left: learn parts from closed tickets, per-appliance issue suggestions.
- **5 Site maps** ☑. Apartments and campgrounds on satellite, streets, an
  uploaded plan, or a blank grid; units linked to unit records and coloured by
  status; campsite attributes; GeoJSON export and import with OSM tags; publish.
  Left: public map page on the listing, print to PDF, snapping.
- **6 Tenant home search** ◐. Filters and sort on the API and home page, and
  tour requests (public form, honeypot, consent, console triage, staff
  notification). Left: map view, gallery, saved searches and alerts, favourites,
  commute filter, calendar hold for a tour.
- **7 House onboarding with autofill** ◐. The property record's type, beds,
  baths and square feet are proposed and applied only when ticked, with an audit
  entry. Left: year built and lot from a live parcel provider, utilities and
  schools, and proposing a rent from comparables.
- **8 Campground reservations** ☐. Follows the map.

---

# Round 2

Checked against the code on 2026-10-01. "Today" lists what exists, with paths
relative to `backend/crates/api/src` unless they start with `frontend/`.

## 9. Go-live hardening ◐

Shipped: list limits and Settings → Schedule (job history, run now). Left:
backups and the restore drill, Playwright journeys, the provider go-live page.

**Today.** `GET /applications`, `GET /my/applications` and `GET /public/listings`
return every row (`routes/applications/list.rs`, `routes/applications/portal.rs`,
`routes/public/listings.rs`). Background jobs run through the durable queue, but
no screen shows what ran, what failed, or when it runs next. Backups depend on the
deploy. Backend tests are strong (unit tests plus an end-to-end integration suite);
the frontend has nine unit-test files and one Playwright spec.

**Design.**
- Page-and-cursor limits on every list that can grow (default 50, max 200), with
  `next` cursors like the audit trail. The public listing search keeps its filters.
- **Settings → Schedule**: every job kind with its schedule, last run, outcome and
  next run; run now; pause; retime within safe bounds. Reads `background_job`.
- Nightly encrypted database backup to object storage, 30-day retention, and a
  documented, timed restore drill on a scratch database.
- Playwright journeys for the paths money and trust run through: sign in, rent
  payment, application, work order to close-out, turnover, owner statement.
- A "go live" checklist page for each provider (`LIVE_PROVIDERS`): keys present,
  test call passed, webhook signature verified.

## 10. Reminders that run themselves ☑

Shipped as fixes F3–F9 in [`FIX-PLAN.md`](FIX-PLAN.md): rent due and past due,
autopay failed, lease expiry with a drafted renewal, inspection reminders with a
calendar file, warranty notices and the managers' morning summary, each sent
once through the notice log and each with its own setting.

**Today.** Six jobs run on their own (`billing_cycle`, `reminder_scan`,
`helpdesk_scan`, `workforce_scan`, `property_photo_scan`, `platform_billing`), but
none sends residents a reminder; `reminder_scan` only tells staff. There is no rent-due notice; the
only late notice is `late_fee_applied`. Autopay failure sends the generic
`payment_failed`. Leases get a staff reminder at 30, 7 and 1 days, but renewals are
proposed by hand. Inspections create no reminder. No calendar invites (ICS), no
warranty-expiry reminder, no manager digest.

**Design.** One `resident_reminders` job per workspace, each rule a setting with an
on/off switch and lead days, every send logged against the lease:
- Rent due (3 days before), rent late (the day after grace, before the fee),
  autopay failed (with a pay-now link), each in the resident's language (area 16).
- Lease expiry at 90/60/30 days: at 90 the renewal workflow opens a **draft**
  proposal with the suggested rent for the manager to send. Nothing reaches the
  resident without a person.
- Inspection, move-in and move-out appointments with an ICS invite
  (`text/calendar`), and a reschedule link.
- Warranty and maintenance-plan due dates from the asset register.
- **Morning digest** for managers: rent late, leases expiring, tickets past SLA,
  turns past target, tours booked today. One email, skipped when empty.

## 11. Vendor portal and compliance ◐

**Today.** Vendors are counterparties. There is a token API for vendor systems
(`routes/vendor`) and the Alpha link, but no portal for a vendor to sign into.
W-9s and insurance certificates are now captured (fix plan F11 and F12): the 1099
reads the TIN, policies are chased before they end, and a workspace can require
current cover before dispatch. The portal (F13) is still to do.

**Design.**
- **`/vendor` portal** (invite by link, like residents): assigned work orders,
  accept / schedule / on the way / done with photos and notes, submit a bill that
  lands in accounts payable for approval.
- **W-9**: legal name, TIN (encrypted with the PII key, shown as last four), tax
  classification and signature; the 1099 export reads it and flags who is missing.
- **COI**: carrier, policy, limits, expiry and the document. Thirty and seven days
  before expiry the vendor gets a request; when expired, dispatch warns and needs
  an override with a reason (audited).
- Alpha vendors keep working through the partner link; the portal is for everyone else.

## 12. Owner portal and spend approvals ☐

**Today.** Owners sign into the full console with the `landlord` role. The data
already exists staff-side: owner statements, payouts, rent roll, T-12, open work
orders, documents. There is no approval threshold on spending.

**Design.**
- **`/owner` portal**: one page per property they own (through their LLCs) with
  this month's money in and out, statements and payouts to download, occupancy,
  open work orders with photos, turns in progress, and documents.
- **Spend approvals**: a per-owner limit (for example $500). A work order, quote or
  vendor bill over it waits for the owner's approve or decline, by portal link or a
  text reply, with an emergency override for staff (audited).
- Monthly statement email with the PDF, from the existing report.

## 13. Texts, round 2 ◐

**Today.** Two-way texts, STOP/START and a shared inbox are shipped. Fix plan
F14–F17 added rating by text, text to work order links, saved replies, assigning
a thread, filing MMS photos, quiet hours and marketing consent. Still missing:
linking an unknown number to a person by hand, and missed-call text-back.

**Design.**
- **Text to work order**: a message that reads like a repair ("sink is leaking")
  gets a one-tap reply link that opens a prefilled request with the photos
  attached; the issue catalog (area 4) suggests the issue.
- **Rate by text**: when a ticket resolves, "How did we do? Reply 1–5"; the reply
  becomes the ticket review that the portal already supports.
- Saved replies, assign to a teammate, link an unknown number to a person, MMS
  photos filed to the resident and the ticket, quiet hours (8 AM–9 PM) for anything
  not urgent, and separate marketing consent.

## 14. Listing media, map search and listing feeds ☐

**Today.** Listings have no photos, so the public site, share images and
structured data have no real picture. Search has filters but no map, no saved
searches and no alerts. "Syndication" in the code is the investor waterfall; there
is no Zillow or Apartments.com feed.

**Design.**
- **Listing photos**: upload, order and caption; the first is the hero; resized
  variants; alt text required. Feeds the listing page, `og:image` and JSON-LD.
- **Map view** of search results (properties already have coordinates) and the
  published site map on the listing page with the unit highlighted.
- **Saved searches and alerts** by email with double opt-in, and favourites.
- **Listing feeds**: a per-workspace XML feed in the common rental listing format
  for portals that accept a feed, plus availability updates when a unit leases.
  Self-showing and lockboxes stay out until a customer asks.

## 15. Operations analytics and the portfolio map ☐

**Today.** Fixed reports exist (rent roll, T-12, aging, delinquency, owner
statement, 1099) and a portfolio summary. Turns now record days vacant and cost;
tickets record SLA, category, appliance and rating; the issue catalog records what
broke. None of it is summarised. The portfolio map (#57) is not built.

**Design.**
- **Operations dashboard**: average days to turn and cost to turn by property and
  month; tickets opened, past SLA and average rating; the issues that repeat by
  property and appliance, with "replace instead of repair" flags where repair spend
  passes a share of replacement cost.
- **Leasing funnel**: tour requests → applications → leases, and days on market.
- **Portfolio map**: every property on one map, coloured by occupancy or open
  work, with the site maps one click away.
- A saved-view report builder only after these show which questions repeat.

## 16. Spanish for everything a resident sees ☐

**Today.** No i18n at all; only the Spanish STOP words are handled.

**Design.** A `language` on people (resident, applicant, vendor), message templates
per language with English fallback, the resident portal and public site translated,
lease and notice PDFs per language. Staff screens stay English. Start with the
templates that send most (rent, maintenance, renewals).

## 17. Family-plan features ☐

From the partnership letter; none started.
- **Related-party guard**: flag transactions between the family's entities; each
  needs a market-rate note and an approver who isn't a party to it.
- **Foundation mode**: income-limit certifications, voucher (HAP) payments split
  from the tenant's share, and the at-cost management fee.
- **Raw land** deal type: acreage, zoning, water and power access, price per acre.

---

## Cross-cutting rules for all of it (both rounds)

- Every write that changes something a customer would ask "who did that?" about
  goes through the audit writer (area 1), with before/after.
- Vantedge support edits are flagged and visible to the customer.
- New tables are tenant-owned with enforced row-level security, like the rest.
- Anything that calls a paid service is sandbox-first (`LIVE_PROVIDERS`).
- Each area ships with an integration test that walks it end to end, and a
  screenshot review in a browser.
