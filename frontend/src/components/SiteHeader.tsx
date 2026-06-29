"use client";

import Link from "next/link";
import { useTheme } from "@/lib/theme";
import { Button } from "@/components/ui/button";

const NAV = [
  { href: "#listings", label: "Listings" },
  { href: "#how-it-works", label: "How it works" },
];

/**
 * Public website header — a sticky, blur-backed top bar that wears the active
 * tenant's white-label brand (mark + company name from the theme). Mirrors the
 * console header's surface/blur treatment.
 */
export default function SiteHeader() {
  const { brand } = useTheme();
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/80 backdrop-blur">
      <div className="mx-auto flex h-[60px] max-w-[1240px] items-center gap-4 px-6">
        <Link href="/" className="flex items-center gap-2.5">
          <span
            className="flex h-[30px] w-[30px] items-center justify-center rounded-[9px] font-display text-lg font-extrabold text-on-accent"
            style={{ background: "var(--accent)" }}
          >
            {brand.company_name.charAt(0) || "A"}
          </span>
          <span className="font-display text-lg font-bold tracking-tight text-ink">
            {brand.company_name}
          </span>
        </Link>

        <nav className="ml-auto hidden items-center gap-1 sm:flex">
          {NAV.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className="rounded-lg px-3 py-2 text-sm font-semibold text-ink-2 transition-colors hover:bg-surface-2 hover:text-ink"
            >
              {item.label}
            </a>
          ))}
        </nav>

        <Button asChild size="sm" className="ml-auto sm:ml-1">
          <Link href="/login">Sign in</Link>
        </Button>
      </div>
    </header>
  );
}
