"use client";

import { ThemeProvider } from "@/lib/theme";
import { AuthProvider } from "@/lib/auth";
import { QueryProvider } from "@/lib/query";
import { Toaster } from "@/components/ui/sonner";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    // QueryProvider is outermost so every client component (including ones that
    // live under ThemeProvider/AuthProvider) can use TanStack Query hooks.
    <QueryProvider>
      <ThemeProvider>
        <AuthProvider>
          {children}
          {/* Toast outlet — Toaster reads the theme, so it sits inside it. */}
          <Toaster position="bottom-right" richColors />
        </AuthProvider>
      </ThemeProvider>
    </QueryProvider>
  );
}
