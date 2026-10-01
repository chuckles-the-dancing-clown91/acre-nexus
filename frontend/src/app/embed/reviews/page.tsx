"use client";

import { EmbedFrame } from "@/components/embed/EmbedFrame";
import { ReviewsStrip } from "@/components/ReviewsStrip";

export default function EmbedReviews() {
  return (
    <EmbedFrame>{({ tenant }) => <ReviewsStrip tenant={tenant} />}</EmbedFrame>
  );
}
