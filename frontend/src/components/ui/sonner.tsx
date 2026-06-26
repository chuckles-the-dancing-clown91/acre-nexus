"use client";

import { Toaster as Sonner, type ToasterProps } from "sonner";

import { useTheme } from "@/lib/theme";

/**
 * Toast system (sonner). Mounted once in `app/providers.tsx`. Reads the Acre
 * theme so toasts follow dark mode, and styles surfaces via the brand tokens.
 * Trigger toasts with `import { toast } from "sonner"`.
 */
export function Toaster(props: ToasterProps) {
  const { dark } = useTheme();
  return (
    <Sonner
      theme={dark ? "dark" : "light"}
      className="toaster group"
      toastOptions={{
        classNames: {
          toast:
            "group toast group-[.toaster]:bg-surface group-[.toaster]:text-ink group-[.toaster]:border-line group-[.toaster]:shadow-acre-lg group-[.toaster]:rounded-2xl",
          description: "group-[.toast]:text-ink-3",
          actionButton:
            "group-[.toast]:bg-accent group-[.toast]:text-on-accent",
          cancelButton: "group-[.toast]:bg-surface-2 group-[.toast]:text-ink-2",
        },
      }}
      {...props}
    />
  );
}
