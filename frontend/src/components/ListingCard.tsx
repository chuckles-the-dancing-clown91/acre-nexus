import Link from "next/link";
import { Bath, BedDouble, Ruler } from "lucide-react";
import type { Listing } from "@/lib/types";
import { Badge, statusTone } from "@/components/ui";

/**
 * Public listings-grid card: a brand-tinted gradient header (white-label accent
 * fading into the subtle surface fill), the home's title + location, a compact
 * beds / baths / sqft spec row, and a prominent monthly rent. Links to the
 * listing detail page.
 */
export function ListingCard({ listing }: { listing: Listing }) {
  const bedsLabel = listing.beds === 0 ? "Studio" : `${listing.beds} bd`;
  return (
    <Link
      href={`/listings/${listing.id}`}
      className="group flex flex-col overflow-hidden rounded-xl border border-line bg-surface shadow-acre transition hover:-translate-y-1 hover:shadow-acre-lg"
    >
      {/* Gradient placeholder header — brand accent into the subtle surface fill. */}
      <div
        className="relative aspect-[3/2]"
        style={{
          background:
            "linear-gradient(150deg, var(--accent-2) 0%, var(--surface-2) 100%)",
        }}
      >
        <div className="absolute left-3 top-3">
          <Badge tone={statusTone(listing.status)}>{listing.status}</Badge>
        </div>
      </div>

      <div className="flex flex-1 flex-col gap-3 p-4">
        <div>
          <h3 className="font-display text-base font-bold tracking-tight text-ink">
            {listing.title}
          </h3>
          <p className="mt-0.5 truncate text-sm text-ink-3">
            {listing.address} · {listing.city}
          </p>
        </div>

        <div className="flex items-center gap-3.5 text-sm font-semibold text-ink-2">
          <span className="inline-flex items-center gap-1.5">
            <BedDouble className="h-4 w-4 text-ink-3" />
            {bedsLabel}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Bath className="h-4 w-4 text-ink-3" />
            {listing.baths} ba
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Ruler className="h-4 w-4 text-ink-3" />
            <span data-numeric>{listing.sqft.toLocaleString()}</span> sqft
          </span>
        </div>

        <div className="mt-auto flex items-baseline gap-1 border-t border-line pt-3">
          <span
            data-numeric
            className="font-display text-2xl font-extrabold tracking-tight text-ink"
          >
            {listing.rent_label}
          </span>
          <span className="text-sm font-semibold text-ink-3">/mo</span>
        </div>
      </div>
    </Link>
  );
}
