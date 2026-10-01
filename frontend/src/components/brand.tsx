// Workspace branding (a client's logo or monogram) and the Vantedge mark that
// signs the product underneath it.

import { cn } from "@/lib/utils";
import type { Brand } from "@/theme/themes";

export const PRODUCT_NAME = "Vantedge";
export const PRODUCT_SLOGAN = "See every angle. Stay a step ahead.";

/** The Vantedge mark: a rising chevron over a horizon line. */
export function VantedgeMark({
  size = 20,
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
      aria-hidden
      className={cn("shrink-0", className)}
    >
      <path
        d="M8 11.5 L14.5 21.5 L24.5 7.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="3.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M7 25 H25"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        opacity="0.5"
      />
    </svg>
  );
}

/** The workspace's logo, or a monogram tile in its accent when it has none. */
export function BrandLogo({
  brand,
  size = 36,
  className,
}: {
  brand: Brand;
  size?: number;
  className?: string;
}) {
  if (brand.logo_url) {
    return (
      // eslint-disable-next-line @next/next/no-img-element -- arbitrary tenant-hosted logo
      <img
        src={brand.logo_url}
        alt={brand.company_name}
        width={size}
        height={size}
        className={cn("shrink-0 rounded-xl object-contain", className)}
      />
    );
  }
  return (
    <span
      aria-hidden
      style={{ width: size, height: size, fontSize: size * 0.46 }}
      className={cn(
        "relative flex shrink-0 items-center justify-center overflow-hidden rounded-[30%] bg-accent font-display font-semibold text-accent-fg",
        "shadow-[inset_0_1px_0_rgb(255_255_255/0.35),0_6px_20px_-8px_var(--accent)]",
        className
      )}
    >
      <span className="absolute inset-0 bg-gradient-to-b from-white/25 to-transparent" />
      <span className="relative">
        {brand.company_name.trim().charAt(0).toUpperCase()}
      </span>
    </span>
  );
}

export function PoweredBy({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 text-xs text-fg-3",
        className
      )}
    >
      <VantedgeMark size={14} className="text-fg-3" />
      Powered by {PRODUCT_NAME}
    </span>
  );
}
