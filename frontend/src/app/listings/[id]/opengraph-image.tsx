// The picture shown when a listing is shared: rent, name, beds and baths, in
// the client's brand colour.

import { ImageResponse } from "next/og";
import { currentTenant, loadListing, loadSite, publicGet } from "@/lib/seo";
import { bedsLabel } from "@/lib/seo-schema";
import type { PublicTheme } from "@/lib/types";

export const alt = "A home for rent";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default async function Image({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const tenant = await currentTenant();
  const [listing, site, theme] = await Promise.all([
    loadListing(tenant, id),
    loadSite(tenant),
    publicGet<PublicTheme>("/public/theme", tenant),
  ]);
  const accent = theme?.accent_color || theme?.primary_color || "#0e7c86";
  // Every element with more than one child needs `display: flex` here.
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        background: accent,
        color: "white",
        padding: 64,
        fontFamily: "sans-serif",
      }}
    >
      <div style={{ display: "flex", fontSize: 34, opacity: 0.9 }}>
        {site.company_name}
      </div>
      {listing ? (
        <div style={{ display: "flex", flexDirection: "column" }}>
          <div
            style={{
              display: "flex",
              alignItems: "baseline",
              fontSize: 110,
              fontWeight: 800,
            }}
          >
            <span>{listing.rent_label}</span>
            <span style={{ fontSize: 42, fontWeight: 600, marginLeft: 12 }}>
              /mo
            </span>
          </div>
          <div
            style={{
              display: "flex",
              fontSize: 56,
              fontWeight: 700,
              marginTop: 8,
            }}
          >
            {listing.title}
          </div>
          <div
            style={{
              display: "flex",
              fontSize: 34,
              marginTop: 12,
              opacity: 0.9,
            }}
          >
            {`${bedsLabel(listing.beds)} · ${listing.baths} bath · ${listing.sqft.toLocaleString()} sqft · ${listing.city}`}
          </div>
        </div>
      ) : (
        <div style={{ display: "flex", fontSize: 64, fontWeight: 800 }}>
          Homes for rent
        </div>
      )}
      <div style={{ display: "flex", fontSize: 30, opacity: 0.85 }}>
        {`Available ${listing?.available_on ?? "now"}`}
      </div>
    </div>,
    size
  );
}
