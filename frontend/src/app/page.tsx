"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { Listing } from "@/lib/types";
import { SiteHeader } from "@/components/SiteHeader";
import { ListingCard } from "@/components/ListingCard";
import { Icon } from "@/components/Icon";

export default function HomePage() {
  const [listings, setListings] = useState<Listing[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .publicListings()
      .then(setListings)
      .catch((e) => setError(e.message));
  }, []);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[1240px] px-6">
        {/* Hero */}
        <section className="grid items-center gap-10 py-16 md:grid-cols-[1.15fr_.85fr]">
          <div className="acre-fade">
            <div className="mb-5 inline-flex items-center gap-2 rounded-full bg-accent-soft px-3 py-1.5 text-xs font-bold text-accent-2">
              <span className="h-1.5 w-1.5 rounded-full bg-accent" />
              {listings
                ? `${listings.length} homes available`
                : "Loading homes…"}
            </div>
            <h1 className="mb-4 font-display text-[clamp(40px,5.2vw,68px)] font-extrabold leading-[.98] tracking-[-.035em]">
              Find a place
              <br />
              you&apos;ll want to <span className="text-accent">stay.</span>
            </h1>
            <p className="mb-6 max-w-[440px] text-[17px] leading-relaxed text-ink-2">
              Browse verified rentals managed by professional teams. Apply once,
              move in faster, and keep everything in one place.
            </p>
            <div className="flex h-[54px] max-w-md items-center gap-2 rounded-2xl border border-line-2 bg-surface pl-4 pr-1.5 shadow-acre">
              <Icon name="search" size={18} className="text-ink-3" />
              <input
                placeholder="City, neighborhood, or ZIP"
                className="w-full border-none bg-transparent text-[15px] outline-none"
              />
              <button className="h-[42px] rounded-xl bg-accent px-5 text-sm font-bold text-on-accent">
                Search
              </button>
            </div>
          </div>
          <div
            className="relative hidden aspect-[4/3.4] overflow-hidden rounded-[22px] shadow-acre-lg md:block"
            style={{
              background: "linear-gradient(150deg,#E9764D,#C5392B 60%,#7c2a1f)",
            }}
          >
            <div className="absolute left-4 top-4 rounded-2xl bg-black/40 px-3.5 py-2.5 text-white backdrop-blur">
              <div className="font-mono text-[11px] opacity-80">
                AVG MOVE-IN
              </div>
              <div className="font-display text-2xl font-bold">6 days</div>
            </div>
          </div>
        </section>

        {/* Listings grid */}
        <section className="pb-20">
          <div className="mb-5 flex items-center justify-between">
            <h2 className="font-display text-2xl font-bold tracking-tight">
              Available homes
            </h2>
            {listings && (
              <span className="text-sm font-semibold text-ink-3">
                {listings.length} listings
              </span>
            )}
          </div>

          {error && (
            <div className="rounded-2xl border border-bad-soft bg-bad-soft p-6 text-bad">
              Couldn&apos;t load listings: {error}. Is the API running on{" "}
              <code>localhost:8000</code>?
            </div>
          )}

          {!listings && !error && (
            <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-72 animate-pulse rounded-2xl border border-line bg-surface-2"
                />
              ))}
            </div>
          )}

          {listings && (
            <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
              {listings.map((l, i) => (
                <ListingCard key={l.id} listing={l} index={i} />
              ))}
            </div>
          )}
        </section>
      </main>
    </>
  );
}
