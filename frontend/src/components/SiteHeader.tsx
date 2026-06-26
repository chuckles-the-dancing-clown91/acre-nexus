"use client";

import Link from "next/link";
import { useTheme } from "@/lib/theme";
import { ThemeToggle } from "./ThemeToggle";

/** Public website header — shows tenant branding (white-label). */
export function SiteHeader() {
  const { brand } = useTheme();
  return (
    <header className="sticky top-0 z-40 flex h-[60px] items-center gap-4 border-b border-line bg-surface/80 px-5 backdrop-blur">
      <Link href="/" className="flex items-center gap-2.5">
        <span
          className="flex h-[30px] w-[30px] items-center justify-center rounded-[9px] font-display text-lg font-extrabold text-on-accent"
          style={{ background: "var(--accent)" }}
        >
          {brand.company_name.charAt(0)}
        </span>
        <span className="font-display text-lg font-bold tracking-tight">
          {brand.company_name}
        </span>
      </Link>
      <nav className="ml-auto flex items-center gap-2">
        <Link
          href="/"
          className="hidden rounded-xl px-3 py-2 text-sm font-semibold text-ink-2 hover:bg-surface-2 sm:block"
        >
          Listings
        </Link>
        <ThemeToggle />
        <Link
          href="/login"
          className="rounded-xl bg-accent px-4 py-2 text-sm font-bold text-on-accent hover:opacity-90"
        >
          Sign in
        </Link>
      </nav>
    </header>
  );
}
