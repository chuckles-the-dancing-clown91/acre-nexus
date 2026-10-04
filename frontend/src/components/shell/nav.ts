"use client";

// The console's navigation model: every destination the current user can
// reach, grouped the way the sidebar shows them. The sidebar, the mobile
// drawer, the command palette, and the breadcrumb all read this one model.

import { useMemo } from "react";
import { useAuth } from "@/lib/auth";
import { useModules } from "@/lib/modules";
import { usePortfolioSummary } from "@/lib/queries";
import { useHasTenantScope, useReach } from "./tenant-scope";
import { MODULES, NAV_GROUPS } from "@/modules/registry";
import type { Tone } from "@/components/ui/badge";

export interface NavItem {
  href: string;
  label: string;
  icon: string;
  /** Match only the exact path (the dashboard), not its children. */
  exact?: boolean;
  preview?: boolean;
  count?: { value: number; tone: Tone };
}

export interface NavGroup {
  key: string;
  label: string;
  items: NavItem[];
}

export const DASHBOARD: NavItem = {
  href: "/console",
  label: "Dashboard",
  icon: "dashboard",
  exact: true,
};

/**
 * Screens that work for someone who sees only their assigned properties. The
 * server closes everything else to them, so the nav doesn't offer it.
 */
const REACH_AWARE = new Set([
  "/console/properties",
  "/console/attention",
  "/console/leases",
  "/console/maintenance",
  "/console/maintenance/schedule",
  "/console/maintenance/kits",
  "/console/my-time",
]);

export function isActive(item: NavItem, pathname: string): boolean {
  return item.exact
    ? pathname === item.href
    : pathname === item.href || pathname.startsWith(`${item.href}/`);
}

export function useNav(): { dashboard: NavItem; groups: NavGroup[] } {
  const { user, can } = useAuth();
  const { isEnabled } = useModules();
  const scoped = useHasTenantScope();
  const { scoped: propertyScoped } = useReach();
  const { data: summary } = usePortfolioSummary({
    enabled: scoped && can("property:read"),
  });

  return useMemo(() => {
    const counts: Record<string, NavItem["count"]> = {};
    if (summary?.open_tickets) {
      counts["/console/maintenance"] = {
        value: summary.open_tickets,
        tone: summary.urgent_tickets ? "bad" : "neutral",
      };
    }
    if (summary?.pending_applications) {
      counts["/console/applications"] = {
        value: summary.pending_applications,
        tone: "warn",
      };
    }

    const moduleGroups: NavGroup[] = NAV_GROUPS.map((g) => ({
      key: g.key,
      label: g.label,
      items: MODULES.filter(
        (m) => m.group === g.key && isEnabled(m.key)
      ).flatMap((m) =>
        m.nav
          .filter((item) => !item.permission || can(item.permission))
          .filter((item) => !propertyScoped || REACH_AWARE.has(item.href))
          .map((item) => ({
            ...item,
            preview: m.preview,
            count: counts[item.href],
          }))
      ),
    }));

    const admin: NavGroup = {
      key: "admin",
      label: "Admin",
      items: [
        !propertyScoped &&
          can("member:read") && {
            href: "/console/members",
            label: "Members",
            icon: "user-cog",
          },
        !propertyScoped &&
          can("billing:read") && {
            href: "/console/billing",
            label: "Billing",
            icon: "wallet",
          },
        !propertyScoped &&
          can("tenant:manage") && {
            href: "/console/modules",
            label: "Modules",
            icon: "blocks",
          },
        !propertyScoped &&
          can("tenant:manage") && {
            href: "/console/settings",
            label: "Settings",
            icon: "settings",
          },
        !propertyScoped &&
          can("audit:read") && {
            href: "/console/audit",
            label: "Audit trail",
            icon: "scroll",
          },
        // Security is per-user (MFA + linked identities), so it has no gate.
        { href: "/console/security", label: "Security", icon: "shield-check" },
      ].filter(Boolean) as NavItem[],
    };

    const platform: NavGroup = {
      key: "platform-admin",
      label: "Vantedge HQ",
      items: user?.is_platform_staff
        ? ([
            {
              href: "/console/platform",
              label: "Platform overview",
              icon: "earth",
              exact: true,
            },
            can("platform:admin") && {
              href: "/console/platform/billing",
              label: "Plans & billing",
              icon: "wallet",
            },
            can("user:read") && {
              href: "/console/platform/users",
              label: "Users",
              icon: "users",
            },
            can("role:read") && {
              href: "/console/platform/roles",
              label: "Roles",
              icon: "shield",
            },
            can("audit:read") && {
              href: "/console/platform/audit",
              label: "Platform audit",
              icon: "scroll",
            },
          ].filter(Boolean) as NavItem[])
        : [],
    };

    return {
      dashboard: DASHBOARD,
      groups: [...moduleGroups, admin, platform].filter(
        (g) => g.items.length > 0
      ),
    };
  }, [user, can, isEnabled, summary, propertyScoped]);
}

/** The nav item (and its group) that best matches a path, for breadcrumbs. */
export function findNavItem(
  groups: NavGroup[],
  pathname: string
): { group: NavGroup | null; item: NavItem } | null {
  if (isActive(DASHBOARD, pathname)) return { group: null, item: DASHBOARD };
  let best: { group: NavGroup; item: NavItem } | null = null;
  for (const group of groups) {
    for (const item of group.items) {
      if (
        isActive(item, pathname) &&
        (!best || item.href.length > best.item.href.length)
      ) {
        best = { group, item };
      }
    }
  }
  return best;
}
