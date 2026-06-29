"use client";

// Theming: dark mode (delegated to next-themes for SSR-safe, no-flicker class
// switching) + per-tenant white-label branding.
//
// Dark mode toggles a `.dark` class on <html> via next-themes (its pre-paint
// inline script sets the class before first paint, so there is no hydration
// flash). White-label branding overrides the --accent / --accent-2 CSS
// variables at runtime from a tenant's theme, so a client can rebrand the whole
// experience (logo + colours + company name) without a rebuild.
//
// Brand is NOT auto-loaded for a hardcoded tenant. The default is Acre's own
// evergreen identity; the console applies the *active workspace's* brand (and
// resets to Acre for the platform/HQ workspace), and the public site applies the
// brand of the tenant being viewed.

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { useTheme as useNextTheme } from "next-themes";
import { api } from "./api";
import type { PublicTheme } from "./types";

interface ThemeCtx {
  /** Resolved dark state (system preference resolved). */
  dark: boolean;
  toggleDark: () => void;
  brand: PublicTheme;
  /** Apply a brand object directly (e.g. live preview in the branding editor). */
  setBrand: (b: PublicTheme) => void;
  /** Load + apply a tenant's brand by slug; `null` resets to the Acre default. */
  setBrandTenant: (slug: string | null) => Promise<void>;
}

const DEFAULT_BRAND: PublicTheme = {
  company_name: "Acre",
  logo_url: null,
  primary_color: "#1f6f47",
  accent_color: "#1f6f47",
  default_mode: "light",
};

const Ctx = createContext<ThemeCtx | null>(null);

/** Override the accent CSS variables from a tenant's brand. */
function applyBrand(brand: PublicTheme) {
  const root = document.documentElement;
  const accent = brand.accent_color || brand.primary_color;
  if (accent) {
    root.style.setProperty("--accent", accent);
    root.style.setProperty("--accent-2", accent);
  }
}

/** Drop any white-label override so the Acre token defaults take over again. */
function clearBrandOverride() {
  const root = document.documentElement;
  root.style.removeProperty("--accent");
  root.style.removeProperty("--accent-2");
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { resolvedTheme, setTheme } = useNextTheme();
  const dark = resolvedTheme === "dark";
  const [brand, setBrandState] = useState<PublicTheme>(DEFAULT_BRAND);

  const setBrand = useCallback((b: PublicTheme) => {
    setBrandState(b);
    applyBrand(b);
  }, []);

  const setBrandTenant = useCallback(async (slug: string | null) => {
    if (!slug) {
      setBrandState(DEFAULT_BRAND);
      clearBrandOverride();
      return;
    }
    try {
      const t = await api.publicTheme(slug);
      setBrandState(t);
      applyBrand(t);
    } catch {
      setBrandState(DEFAULT_BRAND);
      clearBrandOverride();
    }
  }, []);

  const value = useMemo<ThemeCtx>(
    () => ({
      dark,
      toggleDark: () => setTheme(dark ? "light" : "dark"),
      brand,
      setBrand,
      setBrandTenant,
    }),
    [dark, brand, setTheme, setBrand, setBrandTenant]
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useTheme() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}
