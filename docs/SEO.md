# Search (SEO)

Next.js only helps search when pages render on the server. The public pages used
to be client components that fetched in the browser, so a crawler saw an empty
shell with one site-wide title. They are now server-rendered.

## What a crawler gets

- **Home page and every listing page** are rendered on the server with the data
  in the HTML, per workspace (a verified custom domain maps to its workspace
  through `/public/resolve`; otherwise the default workspace).
- **Per page:** title, description, canonical URL, Open Graph and Twitter cards,
  and a generated share image (`/listings/<id>/opengraph-image`, in the client's
  brand colour).
- **Structured data (JSON-LD):** the business as `RealEstateAgent` with contact
  details and `sameAs` profiles, a `WebSite`, an `ItemList` of homes, and each
  listing as a `RealEstateListing` about an `Accommodation` with an `Offer` to
  lease (monthly `UnitPriceSpecification`), plus breadcrumbs. A leased or pending
  home is marked unavailable.
- **`/sitemap.xml`** lists the home page and every public listing with its listed
  date. **`/robots.txt`** allows the public site and points to the sitemap.
- **Kept out of search:** the console, sign-in and account pages, signing links
  and the widget pages send `X-Robots-Tag: noindex, nofollow` and are disallowed
  in `robots.txt`. A missing or unpublished home answers 404 and is not indexed.
- **Search Console:** Settings, Search appearance takes the client's home title,
  description and verification token, with a live preview.

## Setup

- `NEXT_PUBLIC_SITE_URL` fixes the public origin (otherwise it is read from the
  request host). `NEXT_PUBLIC_API_URL` and, optionally, `API_INTERNAL_URL` say
  where the server fetches data. `NEXT_PUBLIC_DEFAULT_TENANT` is the workspace
  for the main host.
- Public data is cached for five minutes on the server, so a new listing
  appears in the sitemap and pages within that time.

## Deliberately not done

- No `aggregateRating` or review markup on the business: Google does not show
  rich results for a business's own reviews on its own site, and marking them up
  invites a manual action. The reviews strip is for visitors.
- No analytics script. Adding GA needs a consent banner first.
- Listings have no photos yet, so no image markup; the share image is generated.
- Embedded widgets are `noindex` on purpose: the content lives on the client's
  page and on the Vantedge listing pages.
