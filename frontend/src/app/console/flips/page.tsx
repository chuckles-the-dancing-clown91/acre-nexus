"use client";

// Flips module page (preview). The board itself is loaded lazily with
// `next/dynamic` — a small demonstration of per-module code-splitting so that
// optional modules don't weigh down the core console bundle.

import dynamic from "next/dynamic";
import { Badge } from "@/components/ui";

const FlipBoard = dynamic(() => import("./components/FlipBoard"), {
  ssr: false,
  loading: () => <div className="text-ink-3">Loading board…</div>,
});

export default function FlipsPage() {
  return (
    <div className="space-y-5">
      <header className="flex items-center gap-3">
        <h1 className="font-display text-2xl font-bold">
          Acquisitions &amp; Flips
        </h1>
        <Badge tone="info">Preview</Badge>
      </header>
      <p className="max-w-2xl text-sm text-ink-3">
        Track buy/flip deals from sourcing through sale. This preview module is
        enabled from <strong>Modules</strong> in settings; the deal domain and
        underwriting tools land next.
      </p>
      <FlipBoard />
    </div>
  );
}
