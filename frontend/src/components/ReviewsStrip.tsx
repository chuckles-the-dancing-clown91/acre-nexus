"use client";

// The public site's Google reviews: rating, count, a few reviews, and the
// link to write one. Renders nothing when the workspace has none to show.

import { useEffect, useState } from "react";
import { business, type PublicReviews } from "@/lib/business";
import { Stars } from "@/components/Stars";

export function ReviewsStrip() {
  const [data, setData] = useState<PublicReviews | null>(null);
  useEffect(() => {
    business
      .publicReviews()
      .then(setData)
      .catch(() => setData(null));
  }, []);

  if (!data || data.rating === null || data.count === 0) return null;
  return (
    <section className="pb-16">
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <h2 className="font-display text-2xl font-bold tracking-tight">
          What residents say
        </h2>
        <span className="flex items-center gap-2 text-sm text-ink-2">
          <Stars rating={data.rating} size={16} />
          <strong>{data.rating.toFixed(1)}</strong>
          <span className="text-ink-3">
            from {data.count} Google review{data.count === 1 ? "" : "s"}
          </span>
        </span>
        {data.write_url && (
          <a
            href={data.write_url}
            target="_blank"
            rel="noopener noreferrer"
            className="ml-auto text-sm font-semibold text-accent-2"
          >
            Write a review
          </a>
        )}
      </div>
      <div className="grid gap-4 md:grid-cols-3">
        {data.reviews.map((r, i) => (
          <figure
            key={i}
            className="rounded-2xl border border-line bg-surface p-5 shadow-acre"
          >
            <Stars rating={r.rating} />
            <blockquote className="mt-2 text-sm leading-relaxed text-ink-2">
              {r.text}
            </blockquote>
            <figcaption className="mt-3 text-xs text-ink-3">
              {r.author}
              {r.when ? ` · ${r.when}` : ""}
            </figcaption>
          </figure>
        ))}
      </div>
    </section>
  );
}
