"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { Bell, ChevronRight, Eye, Menu, Radar, Search, X } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useUiStore } from "@/lib/store";
import { cn } from "@/lib/utils";
import { Tooltip } from "@/components/ui/misc";
import { useTheme } from "@/theme/ThemeProvider";
import { findNavItem, useNav } from "./nav";
import { useActingTenant, useHasTenantScope } from "./tenant-scope";

const iconButton =
  "relative flex size-9 items-center justify-center rounded-xl border border-line bg-fill text-fg-2 backdrop-blur-md transition hover:border-line-strong hover:text-fg [&_svg]:size-4";

export function Topbar() {
  const pathname = usePathname();
  const { groups } = useNav();
  const { user } = useAuth();
  const { hud, setHud } = useTheme();
  const setMobileNavOpen = useUiStore((s) => s.setMobileNavOpen);
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);
  const { acting, set: setActing } = useActingTenant();
  const scoped = useHasTenantScope();
  const crumb = findNavItem(groups, pathname);

  // The inbox lives in a workspace; staff without one have nothing to count.
  const { data: unread } = useQuery({
    queryKey: ["notifications", "unread"],
    queryFn: () => api.unreadCount(),
    enabled: scoped,
    refetchInterval: 60_000,
    refetchOnWindowFocus: true,
    retry: false,
  });
  const count = unread?.unread ?? 0;

  return (
    <header className="sticky top-0 z-20 flex h-16 items-center gap-3 px-4 sm:px-6 lg:px-8">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 -z-10 bg-bg/60 backdrop-blur-xl [mask-image:linear-gradient(to_bottom,black_55%,transparent)]"
      />
      <button
        type="button"
        onClick={() => setMobileNavOpen(true)}
        aria-label="Open navigation"
        className={cn(iconButton, "lg:hidden")}
      >
        <Menu />
      </button>

      <nav
        aria-label="Breadcrumb"
        className="flex min-w-0 items-center gap-1.5 text-[13px]"
      >
        {crumb?.group && (
          <>
            <span className="hidden text-fg-3 sm:inline">
              {crumb.group.label}
            </span>
            <ChevronRight className="hidden size-3.5 text-fg-4 sm:inline" />
          </>
        )}
        <span className="truncate font-medium text-fg">
          {crumb?.item.label ?? "Console"}
        </span>
      </nav>

      <div className="ml-auto flex items-center gap-2">
        {user?.is_platform_staff && acting && (
          <div className="flex h-9 items-center gap-2 rounded-xl border border-info/30 bg-info/10 pr-1 pl-3 text-[12px] text-info">
            <Eye className="size-3.5" />
            <span className="hidden sm:inline">Viewing as</span>
            <span className="font-semibold">{acting}</span>
            <Tooltip content="Stop viewing as this client">
              <button
                type="button"
                onClick={() => setActing(null)}
                aria-label="Stop viewing as this client"
                className="flex size-7 items-center justify-center rounded-lg transition hover:bg-info/15"
              >
                <X className="size-3.5" />
              </button>
            </Tooltip>
          </div>
        )}

        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          aria-label="Search"
          className={cn(iconButton, "lg:hidden")}
        >
          <Search />
        </button>

        <Tooltip content={hud ? "HUD mode on" : "HUD mode off"}>
          <button
            type="button"
            onClick={() => setHud(!hud)}
            aria-pressed={hud}
            aria-label="Toggle HUD mode"
            className={cn(
              iconButton,
              hud &&
                "border-info/40 bg-info/10 text-info hover:border-info/60 hover:text-info"
            )}
          >
            <Radar />
          </button>
        </Tooltip>

        <Tooltip content={count ? `${count} unread` : "Notifications"}>
          <Link
            href="/console/notifications"
            aria-label="Notifications"
            className={iconButton}
          >
            <Bell />
            {count > 0 && (
              <span className="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-accent px-1 font-mono text-[9px] font-semibold text-accent-fg ring-2 ring-bg">
                {count > 99 ? "99+" : count}
              </span>
            )}
          </Link>
        </Tooltip>
      </div>
    </header>
  );
}
