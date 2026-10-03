# Import & export

Moving a portfolio into Vantedge from another tool, and taking the data out.
Console: **Platform → Import & export** (`/console/data`). Module `data`
(on by default). Permissions `data:import` and `data:export` (workspace owners
and admins; property managers scoped to their properties can't import).

## What comes in

| File | What it makes |
| --- | --- |
| **Rent roll** (`tenants`) | Leases with tenant name, email, phone, rent, deposit, start and end, balance and status, plus the properties and units it names when they're new. Vacant rows add the unit. |
| **Properties and units** (`properties`) | One row per unit, or per property for a single-family home. |
| **Owners** (`owners`) | People and companies (a company column or an "LLC", "Inc", "Trust" name makes it a company). |
| **Vendors** (`vendors`) | Contractors, with trades read from columns like "Category" ("Heating & Cooling" → HVAC). |

Files from AppFolio, Buildium, Yardi Breeze, Rent Manager and DoorLoop are
recognised by their columns, and any CSV with a header row works. Each
source's own spellings are matched (`Lease From`, `Lease start`,
`Lease Expiration`, `Tenants`, `Resident`, `Past Due`, `BD/BA` …). The
source guess is a label; the mapping is what counts, and the person importing
sees and can change every field's column before anything is written.

Reading: UTF-8 or Windows-1252 (Excel's CSV), commas or tabs, report title
rows above the header and "Total" rows below are skipped. Money like
`$1,250.00` and `(45.10)`, dates like `7/1/2024`, `07/01/24`, `1-Jul-2024`
and `Jul 1, 2024`, names like `Smith, John`. Up to 5,000 rows and 10 MB a file.

## How it runs

1. **Upload** (`POST /imports?kind=&filename=&source=`, the raw file as the
   body) keeps the file on an `import_batch` draft and returns the guessed
   mapping with a preview.
2. **Map** (`PATCH /imports/<id>` with `mapping`) changes columns; each change
   returns a new preview.
3. **Preview** runs the real import inside a Postgres savepoint and rolls it
   back, so what it shows (new, already here, filled in, problems) is exactly
   what a commit does.
4. **Commit** (`POST /imports/<id>/commit`) runs it for real. Each row has its
   own savepoint: a bad row is reported and skipped, the rest go in. Rows that
   match what's already here (the same property, unit, or lease by tenant and
   start date; the same owner by email or name; the same vendor by name) are
   matched, not doubled; a matched vendor gets missing email, phone or trades
   filled in. The file is dropped from the batch once committed.
5. **Undo** (`POST /imports/<id>/undo`) removes what the batch made, newest
   first, unless something has used it since. What points at a record is read
   from the database catalogue (every table with a `property_id`, `lease_id`,
   … column), so a lease with a payment, a property with a work order or a
   listing, or a vendor on a job stays, and the report says why. Records
   edited after the import stay too.

Imported leases carry their opening balance on the lease (`balance_cents`) and
a note naming the source. The general ledger starts from the import: past
transactions aren't brought in.

## What goes out

`GET /exports` lists the datasets with row counts; `GET /exports/<key>` gives
one as CSV and `GET /exports/all` everything as a zip with a README:
properties and units, tenants and leases, owners, vendors, work orders, and
the general ledger. The first four use the importer's own column names and a
`vantedge_id` column, so they go straight into another workspace (and match
on the way back in rather than duplicating). Cells that would run as a
spreadsheet formula are quoted. Templates: `GET /import-templates/<kind>`.

Every commit, undo and export is in the audit log (`import.commit`,
`import.undo`, `data.export`).
