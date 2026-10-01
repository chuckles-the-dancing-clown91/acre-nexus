"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { motion } from "motion/react";
import { useAuth } from "@/lib/auth";
import { useUiStore } from "@/lib/store";
import { cn } from "@/lib/utils";
import { Ambient } from "@/components/ambient";
import { BrandLogo } from "@/components/brand";
import { useTheme } from "@/theme/ThemeProvider";
import { CommandPalette } from "./CommandPalette";
import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";

export function ConsoleShell({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();
  const router = useRouter();
  const collapsed = useUiStore((s) => s.sidebarCollapsed);

  useEffect(() => {
    if (!loading && !user) router.replace("/login");
  }, [loading, user, router]);

  if (loading || !user) return <Booting />;

  return (
    <>
      <Ambient />
      <aside
        className={cn(
          "glass fixed inset-y-3 left-3 z-30 hidden overflow-hidden rounded-2xl transition-[width] duration-300 ease-out-soft lg:block",
          collapsed ? "w-[68px]" : "w-[252px]"
        )}
      >
        <Sidebar collapsed={collapsed} />
      </aside>
      <MobileNav />
      <CommandPalette />

      <div
        className={cn(
          "min-h-dvh transition-[padding] duration-300 ease-out-soft",
          collapsed ? "lg:pl-[80px]" : "lg:pl-[264px]"
        )}
      >
        <Topbar />
        <main className="mx-auto w-full max-w-[1440px] px-4 pt-2 pb-16 sm:px-6 lg:px-8">
          {children}
        </main>
      </div>
    </>
  );
}

function MobileNav() {
  const open = useUiStore((s) => s.mobileNavOpen);
  const setOpen = useUiStore((s) => s.setMobileNavOpen);
  return (
    <DialogPrimitive.Root open={open} onOpenChange={setOpen}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-black/50 backdrop-blur-[2px] data-[state=open]:animate-[fade-in_180ms_ease-out] lg:hidden" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className="glass-strong fixed inset-y-2 left-2 z-50 w-[min(300px,calc(100vw-3rem))] overflow-hidden rounded-2xl outline-none data-[state=open]:animate-[drawer-in_260ms_var(--ease-out-soft)] lg:hidden"
        >
          <DialogPrimitive.Title className="sr-only">
            Navigation
          </DialogPrimitive.Title>
          <Sidebar onNavigate={() => setOpen(false)} />
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

function Booting() {
  const { brand } = useTheme();
  return (
    <div className="flex min-h-dvh items-center justify-center">
      <Ambient />
      <motion.div
        initial={{ opacity: 0, scale: 0.92 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.4 }}
        className="flex flex-col items-center gap-4"
      >
        <BrandLogo brand={brand} size={44} className="animate-pulse" />
        <span className="eyebrow">Loading workspace</span>
      </motion.div>
    </div>
  );
}
