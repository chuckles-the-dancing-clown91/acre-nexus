import type { Config } from "tailwindcss";
import animate from "tailwindcss-animate";

/**
 * Colours map to CSS custom properties (see globals.css) so the entire palette
 * re-themes for dark mode and per-tenant white-label branding without rebuilding.
 */
const config: Config = {
  content: ["./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: "var(--bg)",
        surface: "var(--surface)",
        "surface-2": "var(--surface-2)",
        raised: "var(--raised)",
        ink: "var(--ink)",
        "ink-2": "var(--ink-2)",
        "ink-3": "var(--ink-3)",
        line: "var(--line)",
        "line-2": "var(--line-2)",
        accent: "var(--accent)",
        "accent-2": "var(--accent-2)",
        "on-accent": "var(--on-accent)",
        "accent-soft": "var(--accent-soft)",
        good: "var(--good)",
        "good-soft": "var(--good-soft)",
        warn: "var(--warn)",
        "warn-soft": "var(--warn-soft)",
        bad: "var(--bad)",
        "bad-soft": "var(--bad-soft)",
        info: "var(--info)",
        "info-soft": "var(--info-soft)",

        // shadcn/ui colour aliases — these reference the same CSS variables as
        // the Acre tokens above (see globals.css), so shadcn components inherit
        // the brand palette and re-theme in dark mode + white-label. Note:
        // `accent` already belongs to the Acre brand token; shadcn's hover
        // "accent" maps to the dedicated `accent-shadcn` utility.
        background: "var(--background)",
        foreground: "var(--foreground)",
        border: "var(--border)",
        input: "var(--input)",
        ring: "var(--ring)",
        card: {
          DEFAULT: "var(--card)",
          foreground: "var(--card-foreground)",
        },
        popover: {
          DEFAULT: "var(--popover)",
          foreground: "var(--popover-foreground)",
        },
        primary: {
          DEFAULT: "var(--primary)",
          foreground: "var(--primary-foreground)",
        },
        secondary: {
          DEFAULT: "var(--secondary)",
          foreground: "var(--secondary-foreground)",
        },
        muted: {
          DEFAULT: "var(--muted)",
          foreground: "var(--muted-foreground)",
        },
        "accent-shadcn": {
          DEFAULT: "var(--shadcn-accent)",
          foreground: "var(--shadcn-accent-foreground)",
        },
        destructive: {
          DEFAULT: "var(--destructive)",
          foreground: "var(--destructive-foreground)",
        },
      },
      fontFamily: {
        display: ["var(--font-display)", "system-ui", "sans-serif"],
        sans: ["var(--font-body)", "system-ui", "sans-serif"],
        mono: ["var(--font-mono)", "monospace"],
      },
      boxShadow: {
        acre: "var(--shadow)",
        "acre-lg": "var(--shadow-lg)",
      },
      borderRadius: {
        xl: "14px",
        "2xl": "18px",
        // shadcn/ui radius scale, derived from --radius.
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
    },
  },
  plugins: [animate],
};

export default config;
