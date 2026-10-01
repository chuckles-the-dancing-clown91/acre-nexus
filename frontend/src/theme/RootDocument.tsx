// The <html> shell every route group's root layout renders. Theme, HUD state,
// and the fitted brand accent are painted server-side so first paint is right.

import "@/app/globals.css";
import { fontVariables } from "@/app/fonts";
import { Providers } from "@/app/providers";
import { accentStyle } from "./accent";
import type { Gate, ThemeName } from "./themes";

export function RootDocument({
  theme,
  gate,
  hud,
  children,
}: {
  theme: ThemeName;
  gate: Gate;
  hud: boolean;
  children: React.ReactNode;
}) {
  return (
    <html
      lang="en"
      data-theme={theme}
      data-hud={hud && theme === "obsidian" ? "on" : "off"}
      className={fontVariables}
      style={accentStyle(gate.brand.accent_color, theme) as React.CSSProperties}
      suppressHydrationWarning
    >
      <body>
        <Providers theme={theme} gate={gate} hud={hud}>
          {children}
        </Providers>
      </body>
    </html>
  );
}
