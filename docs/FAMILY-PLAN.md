# Family-plan features

Roadmap area 17, from the partnership letter: guard deals between the
family's own people and companies, run some entities as a foundation, and
underwrite raw land.

## Related-party guard

A counterparty (Entities, a vendor's page, **Related party**) can be linked to
one of the family's own LLCs or to a family member's owner record. That makes
it a related party.

- A **bill** from a related party gets a review the moment it's raised
  (`related_party_review`, subject `vendor_bill`). The bill can't be approved
  until the review is approved, and a party to it can't approve the bill.
- **Leases and deals** with relatives are flagged by hand (`POST /related-party`,
  subject `lease`, `deal` or `other`, with the owners who are parties).
- Each review needs a **market-rate note** (outside quotes, rent comps, an
  appraisal) and the market price, before it can be approved. The page shows how
  far over or under market it is.
- **Who may decide**: someone with `payable:approve` who is not a party and
  didn't raise it. Parties are the linked owner and every owner of the linked
  LLC, plus any named by hand. A person is matched to their owner record by the
  owner's linked login (`owner.user_id`).

Every flag, note, decision and link is in the audit log with before and after.

Console: Money, **Related parties** (waiting, approved, rejected).

API: `GET /related-party?status`, `POST /related-party`,
`PATCH /related-party/<id>` (`market_cents`, `market_note`),
`POST /related-party/<id>/decide` (`approve`, `note`),
`PUT /entities/<id>/related` (`related_llc_id`, `related_owner_id`).

## Foundation mode

Turned on per LLC (its page, **Foundation**; `tenant:manage`).

- **Income certifications** per lease: household size, yearly income, the area
  median income (AMI) for that size and the limit as a percent of AMI (30, 50,
  60, 80, 120). It qualifies when income is at or under the limit. A
  certification lasts a year unless another end is given. A lease's state is
  `missing`, `ok`, `expiring` (60 days or less), `expired` or `over_limit`.
- **Housing vouchers** per lease: the housing authority, contract number, the
  monthly HAP and the dates it runs. The monthly rent run splits the month into
  the resident's share (`rent`) and the HAP (`hap`), each its own receivable and
  ledger accrual. Late fees only look at the resident's share. Staff record the
  authority's payment with **Received**, which settles it to the operating bank.
- **At-cost management fee**: with fee basis `at_cost`, payouts and owner
  statements charge the staff time spent on the LLC's properties (directly or
  on their work orders) at each person's pay rate plus the labor burden for
  employees. Time already billed to the owner on an in-house bill is left out,
  since that bill is already an expense. Otherwise the fee is the usual percent
  of rent collected. Statements say which (`mgmt_fee_basis`).

Console: Leasing, **Foundation** (every active lease in a foundation LLC with
its certification state and HAP owed), and the lease's **Assistance** tab.

API: `PUT /llcs/<id>/foundation` (`foundation`, `fee_basis`),
`GET /leases/<id>/assistance`, `PUT|DELETE /leases/<id>/voucher`,
`POST /leases/<id>/income-certifications`, `GET /foundation/compliance`,
`POST /lease-payments/<id>/hap-received`.

## Raw land

A deal's type (Deals, the deal page) can be raw land. Land deals track acres,
zoning, water, power and road access, and show the price per acre (offer price,
else asking).

## Not yet

Automatic flags for leases and deals, the HUD income-limit table, and storing
the HAP contract with the voucher.
