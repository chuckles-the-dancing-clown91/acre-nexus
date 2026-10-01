"use client";

import { MotionConfig } from "motion/react";
import { Toaster } from "sonner";
import { TooltipProvider } from "@radix-ui/react-tooltip";
import { QueryProvider } from "@/lib/query";
import { AuthProvider } from "@/lib/auth";
import { ThemeProvider } from "@/theme/ThemeProvider";
import type { Gate, ThemeName } from "@/theme/themes";

export function Providers({
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
    <QueryProvider>
      <ThemeProvider theme={theme} gate={gate} hud={hud}>
        <AuthProvider>
          <MotionConfig reducedMotion="user">
            <TooltipProvider delayDuration={250}>{children}</TooltipProvider>
          </MotionConfig>
          <Toaster
            position="bottom-right"
            theme={theme === "obsidian" ? "dark" : "light"}
            toastOptions={{
              className: "!glass-strong !rounded-2xl !text-fg !font-sans",
            }}
          />
        </AuthProvider>
      </ThemeProvider>
    </QueryProvider>
  );
}
