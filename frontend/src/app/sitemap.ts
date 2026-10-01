// sitemap.xml: the home page and every public listing.

import type { MetadataRoute } from "next";
import { currentTenant, loadListings, siteOrigin } from "@/lib/seo";

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const tenant = await currentTenant();
  const [origin, listings] = await Promise.all([
    siteOrigin(),
    loadListings(tenant),
  ]);
  return [
    { url: origin, changeFrequency: "daily", priority: 1 },
    ...listings.map((l) => ({
      url: `${origin}/listings/${l.id}`,
      lastModified: l.listed_at ? new Date(l.listed_at) : undefined,
      changeFrequency: "daily" as const,
      priority: 0.8,
    })),
  ];
}
