// Brand-accent derivation. A client picks one brand colour for a light
// website; the console renders it on dark glass, so the raw hex is often too
// dark (or too loud) to read. We move it through OKLCH, clamp lightness and
// chroma into a band that works for the theme, and pick a readable foreground.

import type { ThemeName } from "./themes";

type Rgb = [number, number, number];

const toLinear = (c: number) =>
  c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
const toGamma = (c: number) =>
  c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;

export function hexToRgb(hex: string): Rgb | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const h =
    m[1].length === 3
      ? m[1]
          .split("")
          .map((c) => c + c)
          .join("")
      : m[1];
  const n = parseInt(h, 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export function rgbToHex(rgb: Rgb): string {
  return (
    "#" +
    rgb
      .map((c) =>
        Math.round(Math.min(1, Math.max(0, c)) * 255)
          .toString(16)
          .padStart(2, "0")
      )
      .join("")
  );
}

export function rgbToOklch([r, g, b]: Rgb): [number, number, number] {
  const lr = toLinear(r);
  const lg = toLinear(g);
  const lb = toLinear(b);
  const l = Math.cbrt(
    0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb
  );
  const m = Math.cbrt(
    0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb
  );
  const s = Math.cbrt(
    0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb
  );
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  const H = ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360;
  return [L, Math.hypot(A, B), H];
}

function oklchToLinear(L: number, C: number, H: number): Rgb {
  const a = C * Math.cos((H * Math.PI) / 180);
  const b = C * Math.sin((H * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

const inGamut = (rgb: Rgb) => rgb.every((c) => c >= -1e-4 && c <= 1 + 1e-4);

/** OKLCH → sRGB hex, reducing chroma (not lightness) until it fits sRGB. */
export function oklchToHex(L: number, C: number, H: number): string {
  let lin = oklchToLinear(L, C, H);
  if (!inGamut(lin)) {
    let lo = 0;
    let hi = C;
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2;
      if (inGamut(oklchToLinear(L, mid, H))) lo = mid;
      else hi = mid;
    }
    lin = oklchToLinear(L, lo, H);
  }
  return rgbToHex(lin.map((c) => toGamma(Math.max(0, c))) as Rgb);
}

function luminance(rgb: Rgb): number {
  const [r, g, b] = rgb.map(toLinear);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrast(a: string, b: string): number {
  const la = luminance(hexToRgb(a)!);
  const lb = luminance(hexToRgb(b)!);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

const BANDS: Record<ThemeName, { min: number; max: number; hover: number }> = {
  obsidian: { min: 0.72, max: 0.84, hover: 0.05 },
  daylight: { min: 0.44, max: 0.58, hover: -0.05 },
};
const MAX_CHROMA = 0.18;
const INK_DARK = "#071019";
const INK_LIGHT = "#FFFFFF";

export const DEFAULT_ACCENT = "#0E7C86";

export interface AccentPalette {
  accent: string;
  hover: string;
  /** Text/icon colour drawn on top of a solid accent fill. */
  fg: string;
}

/** Fit a brand colour to a theme. Invalid input falls back to the default. */
export function accentPalette(
  hex: string | null | undefined,
  theme: ThemeName
): AccentPalette {
  const rgb = hexToRgb(hex ?? "") ?? hexToRgb(DEFAULT_ACCENT)!;
  const [L, C, H] = rgbToOklch(rgb);
  const band = BANDS[theme];
  const l = Math.min(band.max, Math.max(band.min, L));
  const c = Math.min(C, MAX_CHROMA);
  const accent = oklchToHex(l, c, H);
  const hover = oklchToHex(l + band.hover, c, H);
  const fg =
    contrast(accent, INK_DARK) >= contrast(accent, INK_LIGHT)
      ? INK_DARK
      : INK_LIGHT;
  return { accent, hover, fg };
}

/** The palette as CSS custom properties, for an inline `style` on <html>. */
export function accentStyle(
  hex: string | null | undefined,
  theme: ThemeName
): Record<string, string> {
  const p = accentPalette(hex, theme);
  return {
    "--accent": p.accent,
    "--accent-hover": p.hover,
    "--accent-fg": p.fg,
    "--aura": p.accent,
  };
}
