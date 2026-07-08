# Rehab & Construction

The renovation layer of the investor lifecycle (roadmap Phase 7, issue #40) —
the piece incumbents treat as an afterthought and where flip/BRRRR operators
actually live. Delivered as the pluggable **`rehab`** module
(`docs/MODULES.md`), it tracks a property's rehab **budget**, releases money
against it in **draws** (with **progress photos**), adjusts scope with **change
orders**, and captures **lien waivers** per draw. Money is integer cents; rates
are basis points.

Builds directly on the rest of Phase 7: draws convert a property acquired
through the [deal pipeline](DEALS.md), progress photos ride the
[media / document service](PROPERTY_DATA.md#media-photos--floorplans), and lien
waivers reuse the same text→PDF writer the e-sign envelopes use.

---

## Data model

All tenant-scoped, RLS-enforced (migration `m20240101_000036_rehab`):

| Table | What |
|-------|------|
| `rehab_project` | The budget container on a property: base budget, contingency (bps), status (`planning`/`active`/`complete`/`on_hold`). |
| `rehab_line` | Itemised scope / budget lines (category + budget). |
| `rehab_change_order` | A signed delta to the budget (`pending` → `approved`/`rejected`); approved orders roll into the adjusted budget. |
| `rehab_draw` | A draw request against the budget (`requested` → `approved` → `funded`, or `rejected`), tied to a contractor. |
| `rehab_lien_waiver` | The four statutory waivers (conditional/unconditional × progress/final) captured per draw, each with a generated PDF. |

Progress photos and supporting documents for a draw live in the polymorphic
`document` service with `owner_type = "rehab_draw"`.

### Computed budget roll-up

The project response computes, from the related rows:

- **adjusted budget** = base budget + approved change orders,
- **drawn** = Σ funded draws, **pending draws** = Σ requested/approved draws,
- **remaining** = adjusted budget − drawn,
- **contingency** = base budget × contingency bps, and the itemised **lines
  budget**.

---

## Lien waivers

Requesting a waiver renders the statutory text (type-specific
conditional/unconditional × progress/final language, the contractor, the
property, the amount, and a through-date) to a **PDF via the hand-rolled
text→PDF writer**, files it in the document service against the draw, and
records a `rehab_lien_waiver` (`generated` → `received` once the signed copy is
back). So the platform *produces* a lien waiver per draw, filed and downloadable
like any other document.

---

## API

All under the `rehab` module (JWT; tenant-scoped; self-gated on the module being
enabled), behind `rehab:read` / `rehab:manage`:

| Method | Path | Perm | Description |
|--------|------|------|-------------|
| GET | `/properties/{id}/rehab-projects` | `rehab:read` | Projects on a property (with roll-up) |
| POST | `/properties/{id}/rehab-projects` | `rehab:manage` | Start a rehab budget |
| GET | `/rehab-projects/{id}` | `rehab:read` | Full detail: roll-up + lines + draws + change orders |
| PATCH | `/rehab-projects/{id}` | `rehab:manage` | Edit budget / status / dates |
| POST | `/rehab-projects/{id}/lines` | `rehab:manage` | Add a scope line |
| PATCH · DELETE | `/rehab-lines/{id}` | `rehab:manage` | Edit / remove a scope line |
| POST | `/rehab-projects/{id}/change-orders` | `rehab:manage` | Propose a budget change |
| POST | `/rehab-change-orders/{id}/decide` | `rehab:manage` | Approve / reject (`{ "approve": bool }`) |
| POST | `/rehab-projects/{id}/draws` | `rehab:manage` | Request a draw |
| GET | `/rehab-draws/{id}` | `rehab:read` | A draw with its lien waivers |
| PATCH | `/rehab-draws/{id}/status` | `rehab:manage` | Move `requested → approved → funded` (or `rejected`) |
| POST | `/rehab-draws/{id}/lien-waivers` | `rehab:manage` | Generate a lien-waiver PDF for the draw |
| PATCH | `/rehab-lien-waivers/{id}` | `rehab:manage` | Mark the signed copy `received` |

Every mutation is audited (`rehab.*` actions). Progress photos use the shared
document endpoints with `owner_type=rehab_draw`.

---

## Frontend

`/console/properties/[id]/rehab` (reached from the property profile's **Rehab**
link) is the rehab workspace: a budget summary (adjusted budget / drawn /
pending / remaining + a drawn-vs-budget bar), scope lines (add / remove), change
orders (propose + approve/reject), and draw requests — selecting a draw opens
its progress-photo drawer (the reusable `DocumentsCard`) and its lien-waiver
list with a one-click generator.

Northwind's demo seeds a live project ("Unit turns + roof": $65k budget, an
approved $3k change order, a funded $20k draw, and a generated conditional
progress waiver) so the whole loop renders out of the box.
