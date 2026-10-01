/* eslint-disable @typescript-eslint/no-explicit-any -- JSON-LD graphs are loosely typed */
import { describe, expect, it } from "vitest";
import {
  homeDescription,
  homeJsonLd,
  homeTitle,
  jsonLd,
  listingDescription,
  listingJsonLd,
  listingTitle,
  truncate,
  type SiteInfo,
} from "./seo-schema";
import type { Listing } from "./types";

const site: SiteInfo = {
  slug: "northwind",
  company_name: "Northwind",
  seo_title: null,
  seo_description: null,
  google_site_verification: null,
  phone: "555-0100",
  email: null,
  address: "1 Main St, Portland",
  hours: null,
  website: null,
  same_as: ["https://facebook.com/northwind"],
  logo_url: null,
};

const listing: Listing = {
  id: "abc",
  title: "The Aldercroft Studio",
  address: "15 Alder St",
  city: "Portland",
  beds: 0,
  baths: 1,
  sqft: 520,
  rent_cents: 149500,
  rent_label: "$1,495",
  status: "Available",
  available_on: "Now",
  description: "Sunny studio near the park. ".repeat(20),
  listed_at: "2026-09-01T10:00:00+00:00",
};

describe("truncate", () => {
  it("leaves short text alone and cuts long text on a word", () => {
    expect(truncate("short", 20)).toBe("short");
    const t = truncate("one two three four five six", 15);
    expect(t.length).toBeLessThanOrEqual(15);
    expect(t.endsWith("…")).toBe(true);
    expect(t).not.toMatch(/\s…$/);
  });
});

describe("titles and descriptions", () => {
  it("keeps a listing title and description within search limits", () => {
    expect(listingTitle(listing).length).toBeLessThanOrEqual(52);
    expect(listingTitle(listing)).toContain("Studio");
    const d = listingDescription(listing);
    expect(d.length).toBeLessThanOrEqual(160);
    expect(d).toContain("$1,495");
  });
  it("uses the client's own home title and description when set", () => {
    expect(homeTitle(site)).toBe("Homes for rent | Northwind");
    expect(homeTitle({ ...site, seo_title: "Portland rentals" })).toBe(
      "Portland rentals"
    );
    expect(homeDescription({ ...site, seo_description: "Mine." }, 3)).toBe(
      "Mine."
    );
    expect(homeDescription(site, 3)).toContain("3");
  });
});

describe("structured data", () => {
  it("describes a listing as an offer to lease", () => {
    const g = listingJsonLd(listing, site, "https://homes.example")["@graph"];
    const l = g[0] as Record<string, any>;
    expect(l["@type"]).toBe("RealEstateListing");
    expect(l.url).toBe("https://homes.example/listings/abc");
    expect(l.offers.price).toBe("1495.00");
    expect(l.offers.priceCurrency).toBe("USD");
    expect(l.offers.availability).toBe("https://schema.org/InStock");
    expect(l.about.numberOfBedrooms).toBe(0);
    expect(l.datePosted).toBe("2026-09-01T10:00:00+00:00");
    expect(g[1]["@type"]).toBe("BreadcrumbList");
  });
  it("carries the photos, hero first, and none when there are none", () => {
    const withPhotos = {
      ...listing,
      photos: [
        { url: "https://api.example/p/1", alt: "Porch", caption: null },
        { url: "https://api.example/p/2", alt: "Kitchen", caption: null },
      ],
    };
    const l = listingJsonLd(withPhotos, site, "https://h.example")[
      "@graph"
    ][0] as Record<string, any>;
    expect(l.image).toEqual([
      "https://api.example/p/1",
      "https://api.example/p/2",
    ]);
    const none = listingJsonLd(listing, site, "https://h.example")[
      "@graph"
    ][0] as Record<string, any>;
    expect(none.image).toBeUndefined();
  });
  it("marks a leased home as no longer available", () => {
    const l = listingJsonLd(
      { ...listing, status: "Leased" },
      site,
      "https://h.example"
    )["@graph"][0] as Record<string, any>;
    expect(l.offers.availability).toBe("https://schema.org/SoldOut");
  });
  it("describes the business and lists the homes on the home page", () => {
    const g = homeJsonLd(site, [listing], "https://homes.example")["@graph"];
    const b = g[0] as Record<string, any>;
    expect(b["@type"]).toBe("RealEstateAgent");
    expect(b.telephone).toBe("555-0100");
    expect(b.sameAs).toEqual(["https://facebook.com/northwind"]);
    expect(b.email).toBeUndefined();
    const list = g[2] as Record<string, any>;
    expect(list.numberOfItems).toBe(1);
    expect(list.itemListElement[0].url).toBe(
      "https://homes.example/listings/abc"
    );
  });
  it("cannot break out of a script tag", () => {
    const out = jsonLd({ a: "</script><script>alert(1)</script>" });
    expect(out).not.toContain("</script>");
    expect(JSON.parse(out).a).toContain("</script>");
  });
});
