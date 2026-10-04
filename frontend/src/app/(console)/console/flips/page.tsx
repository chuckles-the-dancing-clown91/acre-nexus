"use client";

// Acquisitions and flips. The board is loaded lazily so an optional module
// doesn't weigh down the rest of the console.

import dynamic from "next/dynamic";
import { Skeleton } from "@/components/ui/misc";

const FlipBoard = dynamic(() => import("./FlipBoard"), {
  ssr: false,
  loading: () => (
    <div className="space-y-6">
      <Skeleton className="h-20 rounded-2xl" />
      <Skeleton className="h-96 rounded-2xl" />
    </div>
  ),
});

export default function FlipsPage() {
  return <FlipBoard />;
}
