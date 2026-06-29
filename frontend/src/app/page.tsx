"use client";

import { useEffect } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { ArrowRight, Home as HomeIcon } from "lucide-react";
import { api, DEFAULT_TENANT } from "@/lib/api";
import { useTheme } from "@/lib/theme";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/page";
import SiteHeader from "@/components/SiteHeader";
import { ListingCard } from "@/components/ListingCard";

export default function HomePage() {
  const { brand, setBrandTenant } = useTheme();

  // Apply the demo tenant's white-label brand for the public experience.
  useEffect(() => {
    setBrandTenant(DEFAULT_TENANT);
  }, [setBrandTenant]);

  const listings = useQuery({
    queryKey: ["public", "listings", DEFAULT_TENANT],
    queryFn: () => api.publicListings(DEFAULT_TENANT),
  });

  const count = listings.data?.length ?? 0;

  return (
    <div className="min-h-screen bg-bg">
      <SiteHeader />

      <main className="mx-auto max-w-[1240px] px-6">
        {/* Hero */}
        <section className="acre-fade grid items-center gap-10 py-16 md:grid-cols-[1.15fr_.85fr]">
          <div>
            <div className="mb-5 inline-flex items-center gap-2 rounded-full bg-accent-soft px-3 py-1.5 text-xs font-bold text-accent-2">
              <span className="h-1.5 w-1.5 rounded-full bg-accent" />
              {listings.isLoading
                ? "Loading homes…"
                : `${count} ${count === 1 ? "home" : "homes"} available`}
            </div>
            <h1 className="mb-4 font-display text-[clamp(40px,5.2vw,64px)] font-extrabold leading-[.98] tracking-[-.035em] text-ink">
              Find a place you&apos;ll
              <br />
              want to <span className="text-accent">stay</span>, with{" "}
              {brand.company_name}.
            </h1>
            <p className="mb-7 max-w-[460px] text-[17px] leading-relaxed text-ink-2">
              Browse verified rentals managed by a professional team. Apply
              once, move in faster, and keep everything in one place.
            </p>
            <div className="flex flex-wrap items-center gap-3">
              <Button asChild size="lg">
                <a href="#listings">
                  Browse listings
                  <ArrowRight className="h-4 w-4" />
                </a>
              </Button>
              <Button asChild size="lg" variant="outline">
                <Link href="/login">Sign in</Link>
              </Button>
            </div>
          </div>

          <div
            className="relative hidden aspect-[4/3.4] overflow-hidden rounded-[22px] shadow-acre-lg md:block"
            style={{
              background:
                "linear-gradient(150deg, var(--accent-2) 0%, var(--accent) 55%, var(--surface-2) 100%)",
            }}
          >
            <div
              className="pointer-events-none absolute inset-0 opacity-[0.08]"
              style={{
                backgroundImage:
                  "radial-gradient(circle at 1px 1px, #fff 1px, transparent 0)",
                backgroundSize: "26px 26px",
              }}
            />
            <div className="absolute left-4 top-4 rounded-2xl bg-black/30 px-3.5 py-2.5 text-white backdrop-blur">
              <div className="font-mono text-[11px] opacity-80">
                AVG MOVE-IN
              </div>
              <div className="font-display text-2xl font-bold" data-numeric>
                6 days
              </div>
            </div>
          </div>
        </section>

        {/* Listings */}
        <section id="listings" className="scroll-mt-20 pb-20">
          <div className="mb-5 flex items-end justify-between">
            <h2 className="font-display text-2xl font-bold tracking-tight text-ink">
              Available homes
            </h2>
            {!listings.isLoading && count > 0 && (
              <span className="text-sm font-semibold text-ink-3">
                {count} {count === 1 ? "listing" : "listings"}
              </span>
            )}
          </div>

          {listings.isLoading ? (
            <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="skeleton h-[340px] rounded-xl"
                  aria-hidden
                />
              ))}
            </div>
          ) : listings.error ? (
            <div className="rounded-xl border border-bad-soft bg-bad-soft p-6 text-sm text-bad">
              Couldn&apos;t load listings:{" "}
              {listings.error instanceof Error
                ? listings.error.message
                : "Something went wrong."}{" "}
              Is the API running on <code className="font-mono">localhost:8000</code>?
            </div>
          ) : count === 0 ? (
            <EmptyState
              icon={HomeIcon}
              title="No homes available right now"
              description="There aren't any listings to show yet. Check back soon — new homes are added regularly."
            />
          ) : (
            <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
              {listings.data!.map((listing) => (
                <ListingCard key={listing.id} listing={listing} />
              ))}
            </div>
          )}
        </section>
      </main>

      {/* Footer */}
      <footer className="border-t border-line bg-surface">
        <div className="mx-auto flex max-w-[1240px] flex-col items-center justify-between gap-3 px-6 py-8 text-sm text-ink-3 sm:flex-row">
          <div className="flex items-center gap-2.5">
            <span
              className="flex h-7 w-7 items-center justify-center rounded-lg font-display text-sm font-extrabold text-on-accent"
              style={{ background: "var(--accent)" }}
            >
              {brand.company_name.charAt(0) || "A"}
            </span>
            <span className="font-semibold text-ink-2">
              {brand.company_name}
            </span>
          </div>
          <p>
            © {new Date().getFullYear()} {brand.company_name}. Professionally
            managed rentals.
          </p>
        </div>
      </footer>
    </div>
  );
}
