// The Vantedge mark and wordmark. The mark is a horizon line under a rising
// chevron — the vantage point (seeing the whole portfolio) meeting the edge
// (staying a step ahead). Colours come from the accent tokens, so a tenant's
// white-label accent carries through.

import { clsx } from "@/lib/clsx";

export const BRAND_NAME = "Vantedge";
export const BRAND_SLOGAN = "See every angle. Stay a step ahead.";
export const BRAND_SLOGAN_SHORT = "See every angle.";

/** The square Vantedge mark. */
export function BrandMark({
  size = 32,
  className,
}: {
  size?: number;
  className?: string;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      role="img"
      aria-label={BRAND_NAME}
      className={clsx("shrink-0", className)}
    >
      <rect width="32" height="32" rx="9" fill="var(--accent)" />
      {/* the rising chevron: a V whose right arm climbs past the horizon */}
      <path
        d="M8 11.5 L14.5 21.5 L24.5 7.5"
        fill="none"
        stroke="var(--on-accent)"
        strokeWidth="3.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* the horizon */}
      <path
        d="M7 25 H25"
        stroke="var(--on-accent)"
        strokeWidth="2"
        strokeLinecap="round"
        opacity="0.55"
      />
    </svg>
  );
}

/** Mark + wordmark, optionally with a product label ("Console") and slogan. */
export function Brand({
  label,
  slogan = false,
  size = 32,
  className,
}: {
  label?: string;
  slogan?: boolean;
  size?: number;
  className?: string;
}) {
  return (
    <span className={clsx("flex items-center gap-2.5", className)}>
      <BrandMark size={size} />
      <span className="flex flex-col leading-tight">
        <span className="font-display text-xl font-bold tracking-tight">
          {BRAND_NAME}
          {label && <span className="font-semibold text-ink-3"> {label}</span>}
        </span>
        {slogan && (
          <span className="text-xs font-medium text-ink-3">{BRAND_SLOGAN}</span>
        )}
      </span>
    </span>
  );
}
