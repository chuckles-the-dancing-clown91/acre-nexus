"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";
import { EmbedFrame } from "@/components/embed/EmbedFrame";
import { TourForm } from "@/components/TourForm";

function Form({ tenant }: { tenant: string }) {
  const listing = useSearchParams().get("listing") || undefined;
  return <TourForm tenant={tenant} listingId={listing} />;
}

export default function EmbedTour() {
  return (
    <EmbedFrame>
      {({ tenant }) => (
        <Suspense fallback={null}>
          <Form tenant={tenant} />
        </Suspense>
      )}
    </EmbedFrame>
  );
}
