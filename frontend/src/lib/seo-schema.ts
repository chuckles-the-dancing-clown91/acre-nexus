// Search-engine markup, kept pure so it is unit-tested: titles, descriptions
// and schema.org JSON-LD for the public site.

import type { Listing } from "@/lib/types";

export interface SiteInfo {
  slug: string;
  company_name: string;
  seo_title: string | null;
  seo_description: string | null;
  google_site_verification: string | null;
  phone: string | null;
  email: string | null;
  address: string | null;
  hours: string | null;
  website: string | null;
  same_as: string[];
  logo_url: string | null;
}

/** Trim to `max` characters on a word boundary, with an ellipsis when cut. */
export function truncate(text: string, max: number): string {
  const t = text.replace(/\s+/g, " ").trim();
  if (t.length <= max) return t;
  const cut = t.slice(0, max - 1);
  const at = cut.lastIndexOf(" ");
  return `${(at > max * 0.6 ? cut.slice(0, at) : cut).replace(/[.,;:\s]+$/, "")}…`;
}

export function bedsLabel(beds: number): string {
  return beds === 0 ? "Studio" : `${beds}-bedroom`;
}

/** "Studio apartment at 15 Alder St, Portland | Northwind" style titles. */
export function listingTitle(l: Listing): string {
  return truncate(
    `${l.title} · ${bedsLabel(l.beds)} for rent in ${l.city}`,
    52
  );
}

export function listingDescription(l: Listing): string {
  const facts = `${bedsLabel(l.beds)}, ${l.baths} bath, ${l.sqft.toLocaleString()} sqft at ${l.address}, ${l.city}. ${l.rent_label} a month, available ${l.available_on}.`;
  return truncate(`${facts} ${l.description}`, 160);
}

export function homeTitle(site: SiteInfo): string {
  return site.seo_title?.trim() || `Homes for rent | ${site.company_name}`;
}

export function homeDescription(site: SiteInfo, count: number): string {
  return (
    site.seo_description?.trim() ||
    truncate(
      `Browse ${count || "our"} verified rental homes from ${site.company_name}. Apply once, tour online, and move in faster.`,
      160
    )
  );
}

/** Safe to place inside a script tag. */
export function jsonLd(data: unknown): string {
  return JSON.stringify(data)
    .replace(/</g, "\\u003c")
    .replace(new RegExp("\\u2028", "g"), "\\u2028")
    .replace(new RegExp("\\u2029", "g"), "\\u2029");
}

export function listingJsonLd(l: Listing, site: SiteInfo, origin: string) {
  const url = `${origin}/listings/${l.id}`;
  return {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "RealEstateListing",
        "@id": `${url}#listing`,
        url,
        name: l.title,
        description: truncate(l.description, 500),
        ...(l.listed_at ? { datePosted: l.listed_at } : {}),
        about: {
          "@type": "Accommodation",
          name: l.title,
          numberOfBedrooms: l.beds,
          numberOfBathroomsTotal: l.baths,
          floorSize: {
            "@type": "QuantitativeValue",
            value: l.sqft,
            unitCode: "FTK",
          },
          address: {
            "@type": "PostalAddress",
            streetAddress: l.address,
            addressLocality: l.city,
          },
        },
        offers: {
          "@type": "Offer",
          price: (l.rent_cents / 100).toFixed(2),
          priceCurrency: "USD",
          businessFunction: "http://purl.org/goodrelations/v1#LeaseOut",
          priceSpecification: {
            "@type": "UnitPriceSpecification",
            price: (l.rent_cents / 100).toFixed(2),
            priceCurrency: "USD",
            unitCode: "MON",
          },
          availability:
            l.status === "Leased" || l.status === "Pending"
              ? "https://schema.org/SoldOut"
              : "https://schema.org/InStock",
          seller: { "@type": "RealEstateAgent", name: site.company_name },
        },
      },
      {
        "@type": "BreadcrumbList",
        itemListElement: [
          { "@type": "ListItem", position: 1, name: "Homes", item: origin },
          { "@type": "ListItem", position: 2, name: l.title, item: url },
        ],
      },
    ],
  };
}

export function homeJsonLd(
  site: SiteInfo,
  listings: Listing[],
  origin: string
) {
  return {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "RealEstateAgent",
        "@id": `${origin}#business`,
        name: site.company_name,
        url: site.website || origin,
        ...(site.logo_url ? { logo: site.logo_url } : {}),
        ...(site.phone ? { telephone: site.phone } : {}),
        ...(site.email ? { email: site.email } : {}),
        ...(site.address
          ? {
              address: {
                "@type": "PostalAddress",
                streetAddress: site.address,
              },
            }
          : {}),
        ...(site.same_as.length ? { sameAs: site.same_as } : {}),
      },
      {
        "@type": "WebSite",
        "@id": `${origin}#website`,
        url: origin,
        name: site.company_name,
        publisher: { "@id": `${origin}#business` },
      },
      {
        "@type": "ItemList",
        name: "Homes for rent",
        numberOfItems: listings.length,
        itemListElement: listings.slice(0, 50).map((l, i) => ({
          "@type": "ListItem",
          position: i + 1,
          url: `${origin}/listings/${l.id}`,
          name: l.title,
        })),
      },
    ],
  };
}
