# Listing syndication

Sending listings to the rental portals. Console: **Leasing → Listings**
(`/console/listings`). Part of the `leasing` module; reading needs
`listing:read`, turning channels on and editing listings `listing:write`.
(Investor syndication, capital calls and waterfalls, is `SYNDICATION.md`.)

## Channels

Portals pull a feed from a URL on their own schedule, so each channel is a
secret feed URL, `/feeds/<channel>/<token>.xml`:

| Channel | Format | Reaches |
| --- | --- | --- |
| `zillow` | Zillow rentals feed (HotPads 2.1 XML) | Zillow, Trulia, HotPads |
| `mits` | MITS 4.1 ILS XML | Apartments.com and the CoStar network, Rent., Zumper, ApartmentList and other listing sites |

A channel is off until someone turns it on, and it can't be turned on without
a leasing email or phone (set on the channel, or taken from the business
profile). Turning a channel on doesn't send anything by itself: the portal has
to be given the URL under an agreement with them (Zillow's feed partner
program; each ILS's partner or support team). The feed works for them from
the first pull. Every pull is logged (`syndication_pull`) with the portal's
user agent, so the console shows when each one last came by. **New URL**
rotates the token; the old URL stops working at once. An off channel, a wrong
token, or a token for another channel is a 404.

## What goes out

A listing is in the feeds when it's public on the website, Available or New
(not Pending or Leased), marked for the portals (`listing.syndicate`, on by
default), and complete: street address, city, two-letter state, ZIP, rent, a
description and at least one photo. The console lists what's missing for each
listing, with advice that doesn't block (a description under 30 words, fewer
than five photos, no square footage). Leasing a home takes it out of the feeds
automatically, since the lease pipeline marks the listing Pending, then Leased.

Each listing carries its photos (stable public URLs that redirect to the
image), beds, baths, square feet, rent, the date it's available (from the
"Now" / "Nov 15" label), the property's coordinates when enrichment found
them, the leasing contact, and a link to its page when the workspace has a
verified website domain. In the MITS feed each property is a `Property` and
each listing a unit on its own floor plan.

Listings gained `state`, `postal_code` and `syndicate` (migration 064;
existing listings take their property's state and ZIP).
