# HOA / Association Management

The `hoa` module (issue #13, *Beyond-GA vertical expansions*) covers community
association management — a distinct buyer from the core PM product. It ships the
core operations loop: **associations**, **members**, dues **assessments**, CC&R
**violations**, and architectural (**ARC**) requests.

Gated by `hoa:read` / `hoa:manage` and the per-tenant `hoa` module toggle. As a
distinct vertical it is **off by default** (preview) — a tenant opts in. Money is
integer cents.

## Model

- **`hoa_association`** — the governing body for a community, optionally tied to a
  `property`, with a standard `dues_cents` at a `dues_frequency`
  (`monthly` / `quarterly` / `annual`).
- **`hoa_member`** — a homeowner / unit in the association.
- **`hoa_assessment`** — a dues charge to a member (recurring or one-off special),
  `due` → `paid` / `void`.
- **`hoa_violation`** — a CC&R violation and its enforcement lifecycle,
  `open` → `cured` / `fined` → `closed`, with an optional fine.
- **`hoa_arc_request`** — an architectural-review request,
  `submitted` → `approved` / `denied` / `withdrawn`.

## API

| Method & path | Permission | Purpose |
|---|---|---|
| `GET  /hoa/associations` | `hoa:read` | Associations + member counts |
| `POST /hoa/associations` | `hoa:manage` | Create an association |
| `GET  /hoa/associations/{id}/members` | `hoa:read` | Homeowners |
| `POST /hoa/associations/{id}/members` | `hoa:manage` | Add a homeowner |
| `POST /hoa/associations/{id}/assessments` | `hoa:manage` | Assess dues to one member, or **bill every active member** |
| `GET  /hoa/associations/{id}/assessments` | `hoa:read` | Dues history |
| `POST /hoa/associations/{id}/violations` | `hoa:manage` | Log a violation |
| `PATCH /hoa/violations/{id}` | `hoa:manage` | Advance the lifecycle / set a fine |
| `GET  /hoa/associations/{id}/violations` | `hoa:read` | Violation log |
| `POST /hoa/associations/{id}/arc-requests` | `hoa:manage` | Submit an ARC request |
| `POST /hoa/arc-requests/{id}/decide` | `hoa:manage` | Approve / deny / withdraw |
| `GET  /hoa/associations/{id}/arc-requests` | `hoa:read` | ARC request log |

Assessing dues without a `member_id` bills every **active** member the amount
(defaulting to the association's standard dues) in one call — the common
period-billing action.

**DoD:** create an association with dues, add members, run a period billing that
assesses every member, log and resolve a violation with a fine, and approve an
ARC request — all tenant-scoped, permission-gated, and audited.

## Schema

Migration `m20240101_000040_hoa` (`hoa_association`, `hoa_member`,
`hoa_assessment`, `hoa_violation`, `hoa_arc_request`), all tenant-owned with
enforced RLS.
