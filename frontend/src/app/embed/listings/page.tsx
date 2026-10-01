"use client";

// Embeddable listings: search, filters and a card grid. Cards open the full
// listing on the Vantedge-hosted site in a new tab.

import { useEffect, useState } from "react";
import { EmbedFrame } from "@/components/embed/EmbedFrame";
import { ListingFilters } from "@/components/ListingFilters";
import { tours, type ListingSearch } from "@/lib/tours";
import type { Listing } from "@/lib/types";

function Grid({ tenant }: { tenant: string }) {
  const [search, setSearch] = useState<ListingSearch>({});
  const [rows, setRows] = useState<Listing[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const t = setTimeout(() => {
      tours
        .search(search, tenant)
        .then((l) => {
          setRows(l);
          setError(null);
        })
        .catch((e: Error) => setError(e.message));
    }, 250);
    return () => clearTimeout(t);
  }, [search, tenant]);

  return (
    <div>
      <ListingFilters value={search} onChange={setSearch} />
      {error && <p className="text-sm text-bad">{error}</p>}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {rows?.map((l) => (
          <a
            key={l.id}
            href={`${window.location.origin}/listings/${l.id}`}
            target="_blank"
            rel="noopener noreferrer"
            className="overflow-hidden rounded-2xl border border-line bg-surface shadow-acre transition hover:-translate-y-0.5"
          >
            <div className="bg-accent px-4 py-6 font-display text-2xl font-extrabold text-on-accent">
              {l.rent_label}
              <span className="text-sm font-semibold opacity-85">/mo</span>
            </div>
            <div className="space-y-1 p-4">
              <div className="font-semibold">{l.title}</div>
              <div className="text-sm text-ink-3">
                {l.address}, {l.city}
              </div>
              <div className="text-sm text-ink-2">
                {l.beds === 0 ? "Studio" : `${l.beds} bd`} · {l.baths} ba ·{" "}
                {l.sqft.toLocaleString()} sqft
              </div>
              <div className="text-xs font-semibold text-accent-2">
                Available {l.available_on}
              </div>
            </div>
          </a>
        ))}
      </div>
      {rows && rows.length === 0 && (
        <p className="py-6 text-center text-sm text-ink-3">
          No homes match. Try widening the search.
        </p>
      )}
    </div>
  );
}

export default function EmbedListings() {
  return <EmbedFrame>{({ tenant }) => <Grid tenant={tenant} />}</EmbedFrame>;
}
