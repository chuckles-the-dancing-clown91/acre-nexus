"use client";

// Acquisitions & Flips module page. The board itself is loaded lazily with
// `next/dynamic` — a small demonstration of per-module code-splitting so that
// optional modules don't weigh down the core console bundle.

import dynamic from "next/dynamic";

const FlipBoard = dynamic(() => import("./components/FlipBoard"), {
  ssr: false,
  loading: () => <div className="text-ink-3">Loading board…</div>,
});

export default function FlipsPage() {
  return (
    <div className="space-y-5">
      <header>
        <h1 className="font-display text-2xl font-bold">
          Acquisitions &amp; Flips
        </h1>
      </header>
      <p className="max-w-2xl text-sm text-ink-3">
        Work buy-side deals from prospecting through close: underwrite each one
        (cap rate, cash-on-cash, IRR, DSCR), keep a due-diligence data room, and
        convert a closed deal into an owned property in one click.
      </p>
      <FlipBoard />
    </div>
  );
}
