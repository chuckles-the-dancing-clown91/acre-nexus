# Campground reservations

A campground or RV park is a site map (see the site maps section of
[ROADMAP-NEXT.md](ROADMAP-NEXT.md), area 5) with kind `campground` or `rv_park`.
Each drawn feature of kind `site` is a bookable site. Its attributes on the map
carry what booking needs:

| attribute | use |
|---|---|
| `site_type` | tent, rv, cabin, glamping. Shown to guests. |
| `rate_cents_night`, `rate_cents_week`, `rate_cents_month` | The price. A stay is priced the cheapest way: whole months, then whole weeks, then nights (a 10 night stay with a weekly rate is one week and three nights, unless ten nights is cheaper). |
| `max_guests`, `max_length_ft` | Checked when booking. |
| `power_amps`, `water`, `sewer`, `pull_through`, `pets` | Shown to guests. |
| `closed` | Not bookable. |

## Booking rules

Per campground (`campground_config`, keyed by map): whether guests can book
online, deposit percent, longest stay, check-in and check-out times, policy text
and add-ons (`{key, label, price_cents, per: stay|night}`).

Seasons (`campground_season`) run from a month-day to a month-day (they can wrap
the year, such as Nov 15 to Feb 15), move the price by a percent and can need a
minimum stay. The price change is averaged over the nights of the stay (two of
four nights in a +20% season is +10%), and the longest minimum of any season the
stay touches applies.

## Stays

`stay` holds site, dates (check-out is the morning the guest leaves), guests,
vehicle and rig length, price lines, deposit, paid amount and status:

```
held (guest request) -> confirmed -> checked_in -> checked_out
held | confirmed -> cancelled
```

`held`, `confirmed` and `checked_in` hold the site. Booking takes a lock on the
site (`pg_advisory_xact_lock`) and refuses a stay overlapping one that holds it,
so two people can't get the same night. Checking out sets the site "to clean"
until someone marks it cleaned.

## Staff

Console, Property, **Campgrounds** (`property:read` to look, `property:write` to
book and act):

- **Front desk**: for a chosen day, requests to confirm, arriving, leaving, in
  house and sites to clean, with confirm, decline, check in, check out, cleaned
  and "paid" (records the balance).
- **Calendar**: two weeks per site, stays as bars.
- **Book a stay**: pick site and dates, see the price, book as confirmed. The
  guest gets the confirmation email with their stay link.
- **Settings**: online booking on or off, deposit, times, policies, add-ons and
  seasons.

API: `GET /campgrounds`, `GET /campgrounds/<id>`, `PUT /campgrounds/<id>/config`,
`POST /campgrounds/<id>/seasons`, `DELETE /campground-seasons/<id>`,
`GET /campgrounds/<id>/availability?from&to` (up to 92 days),
`POST /campgrounds/<id>/quote`, `POST /campgrounds/<id>/stays`,
`GET /campgrounds/<id>/board?date`, `PATCH /stays/<id>`
(`{action: confirm|check_in|check_out|cancel|cleaned|payment, amount_cents?, note?}`).

## Guests

`/camp/<map id>?tenant=<slug>` is the booking page, open when the map is
published and online booking is on. The guest picks dates, sees which sites are
free, picks one, sees the price and sends a request (email required; a hidden
`website` field catches bots). Staff get an email to confirm. The guest gets an
email with `/stay/<token>`, where they see the stay and can cancel before the
arrival day.

API: `GET /public/campgrounds/<id>?from&to`, `POST /public/campgrounds/<id>/quote`,
`POST /public/campgrounds/<id>/stays`, `GET /public/stays/<token>`,
`POST /public/stays/<token>/cancel`.

Emails: `stay_requested`, `stay_confirmed`, `stay_cancelled` to the guest;
`stay_request_staff`, `stay_cancelled_staff` to staff with `property:write`.

## Not yet

Taking the deposit by card online, picking a site on the drawn map, and site
turns on the turnover step engine.
