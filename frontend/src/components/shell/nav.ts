"use client";

// The console's navigation model: every destination the current user can
// reach, grouped the way the sidebar shows them. The sidebar, the mobile
// drawer, the command palette, and the breadcrumb all read this one model.

import { useMemo } from "react";
import { useAuth } from "@/lib/auth";
import { useModules } from "@/lib/modules";
import { usePortfolioSummary } from "@/lib/queries";
import { useHasTenantScope, useReach } from "./tenant-scope";
import { MODULES } from "@/modules/registry";
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
  /** Folded until opened (setup and admin stay out of the way). */
  defaultFolded?: boolean;
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
  "/console/portfolio-map",
  "/console/leases",
  "/console/maintenance",
  "/console/maintenance/schedule",
  "/console/maintenance/plan",
  "/console/maintenance/kits",
  "/console/my-time",
]);

/**
 * The sidebar is organised by the job someone is doing, not by product
 * module: where each destination sits, and in what order.
 */
const SECTIONS: {
  key: string;
  label: string;
  defaultFolded?: boolean;
  items: string[];
}[] = [
  {
    key: "work",
    label: "Work",
    items: [
      "/console/my-day",
      "/console/maintenance",
      "/console/maintenance/plan",
      "/console/maintenance/schedule",
      "/console/maintenance/kits",
      "/console/turns",
      "/console/attention",
      "/console/calendar",
    ],
  },
  {
    key: "properties",
    label: "Properties",
    items: [
      "/console/properties",
      "/console/portfolio-map",
      "/console/maps",
      "/console/campground",
      "/console/properties/onboard",
      "/console/workflows",
      "/console/llcs",
    ],
  },
  {
    key: "leasing",
    label: "Leasing",
    items: [
      "/console/leads",
      "/console/applications",
      "/console/showings",
      "/console/listings",
      "/console/leases",
      "/console/tenant-history",
      "/console/foundation",
      "/console/fees",
    ],
  },
  {
    key: "money",
    label: "Money",
    items: [
      "/console/payments",
      "/console/accounting",
      "/console/payables",
      "/console/related-party",
      "/console/payouts",
      "/console/expenses",
      "/console/reports",
    ],
  },
  {
    key: "people",
    label: "People",
    items: [
      "/console/my-time",
      "/console/team",
      "/console/timesheets",
      "/console/back-office",
    ],
  },
  {
    key: "contacts",
    label: "Contacts",
    items: [
      "/console/crm",
      "/console/entities",
      "/console/messages",
      "/console/texts",
    ],
  },
  { key: "deals", label: "Deals", items: ["/console/flips"] },
  {
    key: "workspace",
    label: "Workspace setup",
    defaultFolded: true,
    items: [
      "/console/onboarding",
      "/console/branding",
      "/console/domains",
      "/console/integrations",
      "/console/go-live",
      "/console/data",
      "/console/tokens",
    ],
  },
  {
    key: "admin",
    label: "Admin",
    defaultFolded: true,
    items: ["/console/notifications"],
  },
];

/** Where a module's group lands when one of its pages isn't listed above. */
const FALLBACK_SECTION: Record<string, string> = {
  property: "properties",
  finance: "money",
  team: "people",
  deals: "deals",
  platform: "workspace",
};

/**
 * Field crew (the maintenance role): they work tickets, schedules and their
 * own time, and see the properties they're sent to. Leases, tenants, money,
 * listings and contacts aren't theirs, so the nav doesn't offer them even
 * though the role may carry read access for ticket context.
 */
export function isFieldCrew(can: (p: string) => boolean): boolean {
  return (
    can("maintenance:read") &&
    ![
      "lease:manage",
      "application:read",
      "listing:read",
      "ledger:read",
      "payable:read",
      "report:read",
      "entity:read",
      "finance:read",
      "message:read",
      "team:manage",
    ].some(can)
  );
}

const FIELD_CREW_ALLOWED = new Set([
  "/console/my-day",
  "/console/maintenance",
  "/console/maintenance/plan",
  "/console/maintenance/schedule",
  "/console/maintenance/kits",
  "/console/turns",
  "/console/attention",
  "/console/calendar",
  "/console/properties",
  "/console/my-time",
  "/console/notifications",
]);

/** Where "home" is for someone: field crew start on their day. */
export function homeHref(can: (p: string) => boolean): string {
  return isFieldCrew(can) ? "/console/my-day" : DASHBOARD.href;
}

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

    const field = isFieldCrew(can);
    const visible: (NavItem & { fallback: string })[] = MODULES.filter((m) =>
      isEnabled(m.key)
    ).flatMap((m) =>
      m.nav
        .filter((item) => !item.permission || can(item.permission))
        .filter((item) => !propertyScoped || REACH_AWARE.has(item.href))
        .filter((item) => !field || FIELD_CREW_ALLOWED.has(item.href))
        // Their home row is My day already.
        .filter((item) => !(field && item.href === "/console/my-day"))
        .map((item) => ({
          ...item,
          preview: m.preview,
          count: counts[item.href],
          fallback: FALLBACK_SECTION[m.group] ?? "workspace",
        }))
    );
    const placed = new Set(SECTIONS.flatMap((sec) => sec.items));
    const moduleGroups: NavGroup[] = SECTIONS.map((sec) => {
      const listed = sec.items
        .map((href) => visible.find((i) => i.href === href))
        .filter((i): i is (typeof visible)[number] => !!i);
      // Anything new that isn't placed yet still shows, at the end of its
      // module's section.
      const extra = visible.filter(
        (i) => !placed.has(i.href) && i.fallback === sec.key
      );
      return {
        key: sec.key,
        label: sec.label,
        defaultFolded: sec.defaultFolded,
        items: [...listed, ...extra].map((i) => {
          const item: NavItem & { fallback?: string } = { ...i };
          delete item.fallback;
          return item;
        }),
      };
    });

    const adminItems: NavItem[] = [
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
    ].filter(Boolean) as NavItem[];
    const admin = moduleGroups.find((g) => g.key === "admin");
    if (admin) admin.items = [...adminItems, ...admin.items];

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
      // Field crew start on their day; everyone else on the dashboard.
      dashboard: field
        ? { href: "/console/my-day", label: "My day", icon: "clock" }
        : DASHBOARD,
      groups: [...moduleGroups, platform].filter((g) => g.items.length > 0),
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
