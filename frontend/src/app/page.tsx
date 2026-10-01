// The public home page, rendered on the server with the listings in it so
// search engines see real content, not an empty shell.

import type { Metadata } from "next";
import { HomeClient } from "@/components/site/HomeClient";
import { currentTenant, loadListings, loadSite, siteOrigin } from "@/lib/seo";
import {
  homeDescription,
  homeJsonLd,
  homeTitle,
  jsonLd,
} from "@/lib/seo-schema";

export async function generateMetadata(): Promise<Metadata> {
  const tenant = await currentTenant();
  const [site, listings, origin] = await Promise.all([
    loadSite(tenant),
    loadListings(tenant),
    siteOrigin(),
  ]);
  const title = homeTitle(site);
  const description = homeDescription(site, listings.length);
  return {
    title: { absolute: title },
    description,
    alternates: { canonical: origin },
    openGraph: {
      type: "website",
      url: origin,
      title,
      description,
      siteName: site.company_name,
    },
    twitter: { card: "summary_large_image", title, description },
  };
}

export default async function HomePage() {
  const tenant = await currentTenant();
  const [site, listings, origin] = await Promise.all([
    loadSite(tenant),
    loadListings(tenant),
    siteOrigin(),
  ]);
  return (
    <>
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{
          __html: jsonLd(homeJsonLd(site, listings, origin)),
        }}
      />
      <HomeClient initialListings={listings} />
    </>
  );
}
