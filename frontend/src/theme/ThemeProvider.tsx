"use client";

// Client side of the theme system. The server root layout has already painted
// <html> with the gate's theme, brand accent, and HUD preference, so nothing
// here runs on first paint; this context only exposes those values and lets the
// user flip HUD intensity (or a branding screen preview a new accent) live.

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { accentStyle } from "./accent";
import { HUD_COOKIE, type Brand, type Gate, type ThemeName } from "./themes";

interface ThemeCtx {
  theme: ThemeName;
  gate: Gate;
  brand: Brand;
  /** Swap the accent at runtime, e.g. while previewing branding edits. */
  setBrand: (brand: Brand) => void;
  hud: boolean;
  setHud: (on: boolean) => void;
}

const Ctx = createContext<ThemeCtx | null>(null);

export function ThemeProvider({
  theme,
  gate,
  hud: initialHud,
  children,
}: {
  theme: ThemeName;
  gate: Gate;
  hud: boolean;
  children: React.ReactNode;
}) {
  const [brand, setBrandState] = useState(gate.brand);
  const [hud, setHudState] = useState(initialHud);

  const setHud = useCallback((on: boolean) => {
    setHudState(on);
    document.documentElement.dataset.hud = on ? "on" : "off";
    document.cookie = `${HUD_COOKIE}=${on ? 1 : 0}; path=/; max-age=31536000; samesite=lax`;
  }, []);

  const setBrand = useCallback(
    (next: Brand) => {
      setBrandState(next);
      const root = document.documentElement;
      for (const [k, v] of Object.entries(
        accentStyle(next.accent_color, theme)
      )) {
        root.style.setProperty(k, v);
      }
    },
    [theme]
  );

  const value = useMemo(
    () => ({ theme, gate, brand, setBrand, hud, setHud }),
    [theme, gate, brand, setBrand, hud, setHud]
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useTheme() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}
