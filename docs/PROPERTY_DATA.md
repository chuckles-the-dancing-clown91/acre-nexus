# Property Intelligence

"Zillow but better": every property carries rich, structured data — parcel /
county records, tax history, an automated valuation (AVM) + rent estimate,
assigned schools, and utility providers — that is **fetched and validated
automatically** by background workers rather than hand-entered.

This is delivered as the pluggable **`property_intel`** module
(`docs/MODULES.md`), so a tenant can turn it on/off, and it is on by default.

---

## Data model

All tables are tenant-scoped and keyed to a property (`entity/` + migration
`m20240101_000007_property_data`):

| Table | Cardinality | What |
|-------|-------------|------|
| `property_detail` | 1 per property | Physical attrs (beds/baths/sqft/lot/type/stories/parking/HVAC), geo (lat/long, matched address), and the **parcel / county record** (APN, zoning, subdivision, county, FIPS, owner of record, last sale, flood zone, walk score). |
| `property_tax` | per year | Assessment history: assessed / land / improvement value, tax amount, effective rate (bps). |
| `property_valuation` | history | AVM snapshots: estimated value with a low/high band, estimated rent, confidence. |
| `property_school` | per level | Assigned/nearby schools: level, district, rating, distance, grades. |
| `property_utility` | per type | Electric / gas / water / sewer / trash / internet provider + typical cost. |
| `enrichment_run` | per fetch | The observable trail: which source, which provider, succeeded/failed, linked job, detail. |

Money is integer cents throughout; the API also returns formatted `*_label`s.

---

## The enrichment engine

Code lives in `api/src/enrichment/`, one responsibility per file:

| File | Responsibility |
|------|----------------|
| `source.rs` | the source taxonomy + job-kind mapping |
| `data.rs` | provider output shapes + the error type |
| `geocode.rs` | the **live** U.S. Census geocoder provider |
| `simulated.rs` | deterministic simulated providers (parcel/tax/valuation/schools/utilities) |
| `runner.rs` | call a provider for one source, persist the result, return a summary |

### Providers (pluggable) + graceful fallback

Every source sits behind the same interface. One is a **real** integration —
the free, keyless **U.S. Census geocoder** (`geocode.rs`). It calls the
**geographies** endpoint, so a live geocode returns not just coordinates + a
normalised matched address but the **real county and county FIPS** — genuine
government data, not a stand-in. The remaining sources are **deterministic
simulated** providers seeded from the property, so the state machine and
durability are real while CI stays hermetic and repeated runs are idempotent.
Replacing a simulated source with a real API (county assessor, an AVM vendor,
GreatSchools, …) is a one-function change.

**Graceful fallback (roadmap Phase 7 DoD).** A live provider that is
unavailable does **not** fail the job — the runner falls back to the simulated
provider so the property still gets enriched, and records *which provider
actually served the source* on the `enrichment_run` (`provider` = `census_geocoder`
vs `simulated`, plus `detail.fell_back` + a reason). So a real address enriches
from live sources, and degrades cleanly to simulation when the source can't be
reached. Only a *real* failure (e.g. the database) is retried/failed by the
scheduler. Real, credentialed vendors slot in per-source behind the same
`LIVE_PROVIDERS` gate the payments/screening providers already use.

> Networking note: in this managed environment outbound HTTPS goes through an
> agent proxy that MITMs TLS, so the geocoder client trusts the proxy CA bundle
> when present and picks up `HTTPS_PROXY` automatically.

---

## How it runs — the durable queue

Work is driven by the Tokio scheduler's durable job queue (`background_job`),
now hardened into a proper retrying queue:

- `max_attempts` + `last_error` columns, exponential backoff, and a terminal
  `failed` state (`JobOutcome::retry` / `JobOutcome::failed`).
- A transient failure (e.g. the geocoder is briefly unreachable) retries with
  backoff; once the budget is exhausted the job is marked `failed` and an
  `enrichment_run` records why.

Flow:

```
POST /properties/{id}/enrich
        │  enqueue
        ▼
  enrich_property (orchestrator)  ──fans out──▶  enrich_geocode
                                                 enrich_parcel
                                                 enrich_tax
                                                 enrich_valuation
                                                 enrich_schools
                                                 enrich_utilities
        each child job → runner::run_source → writes its table(s) + enrichment_run
```

Each source is its own job, so they run and retry independently. The
`property_intel` module (`api/src/modules/enrichment.rs`) owns these job kinds.

---

## API

All under the `property_intel` module (JWT; tenant-scoped):

| Method | Path | Permission | Description |
|--------|------|-----------|-------------|
| GET | `/properties/{id}/intel` | `property:read` | Aggregated detail + valuations + taxes + schools + utilities |
| POST | `/properties/{id}/enrich` | `property:write` | Enqueue enrichment (body `{ "sources": [...] }`, omit for all). Audited as `property.enrich`. Requires the module enabled. |
| GET | `/properties/{id}/enrichment` | `property:read` | Recent enrichment runs (newest first, each with the provider used + fallback flag) |
| GET | `/properties/{id}/media` | `property:read` | Property photos + floorplans, each with a fresh signed URL; plus the hero |
| PATCH | `/properties/{id}/hero` | `property:write` | Promote a media document to the hero photo (`{ "document_id": … }`), or clear it with `null` |

`POST /properties/{id}/enrich` accepts any subset of
`geocode`, `parcel`, `tax`, `valuation`, `schools`, `utilities`; an empty/omitted
list refreshes all. It returns the orchestrator `job_id` and the `scheduled`
sources.

---

---

## Media (photos / floorplans)

Property **media** rides the polymorphic [`document`](INTEGRATIONS.md) service —
`owner_type = "property"`, category `photo` / `floorplan` — so photos share the
upload / versioning / signed-URL / retention machinery of every other document.

- `GET /properties/{id}/media` returns the property's image documents, each with
  a **fresh short-lived signed GET URL** the console renders inline in an
  `<img>`, newest first.
- The **hero** photo is stored as a stable `doc:{id}` sentinel in
  `property.image_url`; the profile builder (and the media endpoint) resolve it
  to a fresh signed URL on every read, so the hero never points at a URL that has
  since expired. `PATCH /properties/{id}/hero` sets or clears it.

The property profile has a **Media** tab: an image gallery with upload and
"set as hero"; the resolved hero shows in the profile header. Northwind's demo
seeds a hero + gallery photo on two properties so the feature renders out of the
box.

---

## Frontend

The property profile page (`/console/properties/[id]`) renders the parcel /
county record, the AVM valuation + rent estimate, the tax history table, schools,
and utilities, with an **Enrich data** button that triggers the queue and
refreshes as jobs complete, plus a **Media** tab for photos/floorplans. Demo
data for two properties is populated at seed time via the engine's simulated
providers (and the live geocoder resolves real coordinates + county on demand).
