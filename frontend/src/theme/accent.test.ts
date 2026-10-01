import { describe, expect, it } from "vitest";
import {
  accentPalette,
  contrast,
  DEFAULT_ACCENT,
  hexToRgb,
  oklchToHex,
  rgbToOklch,
} from "./accent";

const lightness = (hex: string) => rgbToOklch(hexToRgb(hex)!)[0];

describe("hex parsing", () => {
  it("accepts 3- and 6-digit hex with or without #", () => {
    expect(hexToRgb("#fff")).toEqual([1, 1, 1]);
    expect(hexToRgb("000000")).toEqual([0, 0, 0]);
  });

  it("rejects anything else", () => {
    expect(hexToRgb("teal")).toBeNull();
    expect(hexToRgb("#12345")).toBeNull();
  });

  it("round-trips through OKLCH", () => {
    for (const hex of ["#0e7c86", "#e9764d", "#3a6ea5", "#808080"]) {
      const [l, c, h] = rgbToOklch(hexToRgb(hex)!);
      expect(oklchToHex(l, c, h)).toBe(hex);
    }
  });
});

describe("accentPalette", () => {
  const brands = [
    "#0E7C86",
    "#E9764D",
    "#1A1A1A",
    "#FFE600",
    "#7C3AED",
    "#00F0FF",
    "#C5392B",
  ];

  it("lifts dark brand colours so they read on obsidian", () => {
    for (const b of brands)
      expect(
        lightness(accentPalette(b, "obsidian").accent)
      ).toBeGreaterThanOrEqual(0.715);
  });

  it("deepens light brand colours so they read on daylight", () => {
    for (const b of brands)
      expect(
        lightness(accentPalette(b, "daylight").accent)
      ).toBeLessThanOrEqual(0.585);
  });

  it("always picks a foreground with WCAG AA contrast on the accent", () => {
    for (const theme of ["obsidian", "daylight"] as const) {
      for (const b of brands) {
        const p = accentPalette(b, theme);
        expect(contrast(p.accent, p.fg)).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("falls back to the default accent for invalid input", () => {
    expect(accentPalette("not a colour", "obsidian")).toEqual(
      accentPalette(DEFAULT_ACCENT, "obsidian")
    );
    expect(accentPalette(null, "daylight")).toEqual(
      accentPalette(DEFAULT_ACCENT, "daylight")
    );
  });
});
