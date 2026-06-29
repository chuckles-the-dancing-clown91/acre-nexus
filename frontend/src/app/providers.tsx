"use client";

import { ThemeProvider as NextThemes } from "next-themes";
import { ThemeProvider } from "@/lib/theme";
import { AuthProvider } from "@/lib/auth";
import { QueryProvider } from "@/lib/query";
import { Toaster } from "@/components/ui/sonner";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    // QueryProvider is outermost so every client component can use TanStack
    // Query hooks. NextThemes owns the .dark class (SSR-safe, no flash); the
    // Acre ThemeProvider layers white-label branding on top and re-exposes a
    // stable useTheme() API.
    <QueryProvider>
      <NextThemes
        attribute="class"
        defaultTheme="light"
        enableSystem
        disableTransitionOnChange
      >
        <ThemeProvider>
          <AuthProvider>
            {children}
            <Toaster position="bottom-right" richColors />
          </AuthProvider>
        </ThemeProvider>
      </NextThemes>
    </QueryProvider>
  );
}
