// The listing page, rendered on the server so search engines and link
// previews get the whole thing: title, description, canonical link, Open
// Graph card and schema.org markup.

import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ListingView } from "@/components/site/ListingView";
import { currentTenant, loadListing, loadSite, siteOrigin } from "@/lib/seo";
import {
  jsonLd,
  listingDescription,
  listingJsonLd,
  listingTitle,
} from "@/lib/seo-schema";

type Props = { params: Promise<{ id: string }> };

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { id } = await params;
  const tenant = await currentTenant();
  const listing = await loadListing(tenant, id);
  // A missing or unpublished home is not indexed.
  if (!listing) return { title: "Home not found", robots: { index: false } };
  const origin = await siteOrigin();
  const url = `${origin}/listings/${listing.id}`;
  const title = listingTitle(listing);
  const description = listingDescription(listing);
  return {
    title,
    description,
    alternates: { canonical: url },
    openGraph: {
      type: "website",
      url,
      title,
      description,
      siteName: (await loadSite(tenant)).company_name,
    },
    twitter: { card: "summary_large_image", title, description },
  };
}

export default async function ListingPage({ params }: Props) {
  const { id } = await params;
  const tenant = await currentTenant();
  const listing = await loadListing(tenant, id);
  if (!listing) notFound();
  const [site, origin] = await Promise.all([loadSite(tenant), siteOrigin()]);
  return (
    <>
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{
          __html: jsonLd(listingJsonLd(listing, site, origin)),
        }}
      />
      <ListingView listing={listing} />
    </>
  );
}
