import Link from "next/link";
import type { Listing } from "@/lib/types";
import { gradFor } from "@/lib/gradients";
import { Badge, statusTone } from "@/components/ui";

/** Pluggable listing card used on the public website grid. */
export function ListingCard({ listing, index }: { listing: Listing; index: number }) {
  const bedsLabel = listing.beds === 0 ? "Studio" : `${listing.beds} bd`;
  return (
    <Link
      href={`/listings/${listing.id}`}
      className="group overflow-hidden rounded-2xl border border-line bg-surface shadow-acre transition hover:-translate-y-1 hover:shadow-acre-lg"
    >
      <div
        className="relative aspect-[3/2.1]"
        style={{ background: gradFor(index) }}
      >
        <div className="absolute left-3 top-3">
          <Badge tone={statusTone(listing.status)}>{listing.status}</Badge>
        </div>
        <div className="absolute bottom-3 left-4 font-display text-2xl font-extrabold text-white">
          {listing.rent_label}
          <span className="text-sm font-semibold opacity-85">/mo</span>
        </div>
      </div>
      <div className="p-4">
        <div className="text-base font-bold tracking-tight">{listing.title}</div>
        <div className="mb-3 mt-0.5 text-sm text-ink-3">
          {listing.address} · {listing.city}
        </div>
        <div className="flex gap-3.5 text-sm font-semibold text-ink-2">
          <span>{bedsLabel}</span>
          <span>{listing.baths} ba</span>
          <span>{listing.sqft.toLocaleString()} sqft</span>
        </div>
      </div>
    </Link>
  );
}
