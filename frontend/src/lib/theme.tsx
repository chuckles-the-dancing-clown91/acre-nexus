"use client";

// Theming system: dark-mode toggle + per-tenant white-label branding.
//
// Dark mode toggles a `.dark` class on <html> (CSS variables flip in globals.css).
// White-label branding overrides the --accent / --accent-2 / --on-accent CSS
// variables at runtime from the tenant's theme, so a client can rebrand the
// entire experience (logo + colours + company name) without a rebuild.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { api, DEFAULT_TENANT } from "./api";
import type { PublicTheme } from "./types";

interface ThemeCtx {
  dark: boolean;
  toggleDark: () => void;
  brand: PublicTheme;
  setBrand: (b: PublicTheme) => void;
}

const DEFAULT_BRAND: PublicTheme = {
  company_name: "Acre",
  logo_url: null,
  primary_color: "#F5451F",
  accent_color: "#F5451F",
  default_mode: "light",
};

const Ctx = createContext<ThemeCtx | null>(null);

/** Apply a tenant's brand colours by overriding the accent CSS variables. */
function applyBrand(brand: PublicTheme) {
  const root = document.documentElement;
  const accent = brand.accent_color || brand.primary_color;
  if (accent) {
    root.style.setProperty("--accent", accent);
    root.style.setProperty("--accent-2", accent);
  }
}

export function ThemeProvider({
  children,
  tenant = DEFAULT_TENANT,
}: {
  children: React.ReactNode;
  tenant?: string;
}) {
  const [dark, setDark] = useState(false);
  const [brand, setBrandState] = useState<PublicTheme>(DEFAULT_BRAND);

  // Restore dark preference.
  useEffect(() => {
    const saved = localStorage.getItem("acre.dark");
    if (saved === "1") setDark(true);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("acre.dark", dark ? "1" : "0");
  }, [dark]);

  // Load tenant branding for white-label.
  useEffect(() => {
    let cancelled = false;
    api
      .publicTheme(tenant)
      .then((t) => {
        if (cancelled) return;
        setBrandState(t);
        applyBrand(t);
      })
      .catch(() => {
        /* fall back to default brand */
      });
    return () => {
      cancelled = true;
    };
  }, [tenant]);

  const setBrand = (b: PublicTheme) => {
    setBrandState(b);
    applyBrand(b);
  };

  const value = useMemo<ThemeCtx>(
    () => ({ dark, toggleDark: () => setDark((d) => !d), brand, setBrand }),
    [dark, brand]
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useTheme() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}
