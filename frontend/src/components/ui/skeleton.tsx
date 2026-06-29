import * as React from "react";
import { cn } from "@/lib/utils";

/**
 * Shimmer placeholder. The `.skeleton` class (surface-2 background + animated
 * shimmer, respecting prefers-reduced-motion) is defined in globals.css.
 */
function Skeleton({ className }: { className?: string }) {
  return <div className={cn("skeleton", className)} />;
}

/**
 * Stack of skeleton text bars. Renders `lines` bars at h-4; the last bar is
 * narrowed to w-2/3 to mimic a ragged final line of text.
 */
function SkeletonText({
  lines = 3,
  className,
}: {
  lines?: number;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton
          key={i}
          className={cn("h-4 w-full", i === lines - 1 && "w-2/3")}
        />
      ))}
    </div>
  );
}

export { Skeleton, SkeletonText };
