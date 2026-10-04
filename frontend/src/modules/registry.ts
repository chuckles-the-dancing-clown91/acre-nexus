// Frontend module registry.
//
// Mirrors the backend `crate::modules` registry: each entry's `key` matches a
// backend `ModuleManifest.key`, so the two agree on what a module is, what
// permission gates it, and whether it ships on by default. This registry drives
// the console navigation, the module settings screen, and per-module route
// gating — adding a module here lights up its nav entry everywhere.

/** A navigation entry contributed by a module. */
export interface ModuleNavItem {
  /** Console route, e.g. `/console/properties`. */
  href: string;
  label: string;
  /** Icon key understood by `<Icon />`. */
  icon: string;
  /** Permission required to see this item (omit for "any signed-in user"). */
  permission?: string;
}

/** Sidebar section a module's nav items are grouped under. */
export type NavGroupKey =
  "property" | "finance" | "team" | "deals" | "platform";

/** A pluggable product module as seen by the frontend. */
export interface ModuleDef {
  /** Stable key, identical to the backend module key. */
  key: string;
  label: string;
  description: string;
  /** Sidebar section this module's nav items are grouped under. */
  group: NavGroupKey;
  /** Navigation entries this module adds to the console sidebar. */
  nav: ModuleNavItem[];
  /** Whether the module is on for a tenant with no explicit override. */
  defaultEnabled: boolean;
  /** Preview modules are off by default and badged in the UI. */
  preview?: boolean;
}

/** Sidebar section headers, in display order. */
export const NAV_GROUPS: { key: NavGroupKey; label: string }[] = [
  { key: "property", label: "Property" },
  { key: "finance", label: "Finance" },
  { key: "team", label: "Team" },
  { key: "deals", label: "Deals" },
  { key: "platform", label: "Platform & integrations" },
];

/** Every module the frontend knows how to render. */
export const MODULES: ModuleDef[] = [
  {
    key: "properties",
    label: "Properties & Portfolio",
    description: "Portfolio, property profiles, and LLC holding entities.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/properties",
        label: "Properties",
        icon: "building",
        permission: "property:read",
      },
      {
        href: "/console/attention",
        label: "Needs attention",
        icon: "alert",
        permission: "property:read",
      },
      {
        href: "/console/portfolio-map",
        label: "Portfolio map",
        icon: "earth",
        permission: "property:read",
      },
      {
        href: "/console/properties/onboard",
        label: "Onboard",
        icon: "house-plus",
        permission: "property:write",
      },
      {
        href: "/console/maps",
        label: "Site maps",
        icon: "map",
        permission: "property:read",
      },
      {
        href: "/console/workflows",
        label: "Workflows",
        icon: "workflow",
        permission: "property:read",
      },
      {
        href: "/console/llcs",
        label: "LLCs",
        icon: "landmark",
        permission: "property:read",
      },
      {
        href: "/console/onboarding",
        label: "Getting set up",
        icon: "rocket",
        permission: "tenant:manage",
      },
    ],
  },
  {
    key: "entities",
    label: "Entities & Contacts",
    description:
      "Registry of banks, lenders, contractors and other counterparties, with notes.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/entities",
        label: "Entities",
        icon: "briefcase",
        permission: "entity:read",
      },
      {
        href: "/console/crm",
        label: "Owners & CRM",
        icon: "handshake",
        permission: "entity:read",
      },
    ],
  },
  {
    key: "rentals",
    label: "Rentals & Leasing",
    description: "Units, leases/tenancies, and the rent ledger.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/leases",
        label: "Tenants",
        icon: "users",
        permission: "lease:read",
      },
    ],
  },
  {
    key: "accounting",
    label: "Accounting & Payments",
    description:
      "Double-entry ledger per LLC, rent collection (cards/ACH with autopay), late fees, bank reconciliation, and owner payouts.",
    group: "finance",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/payments",
        label: "Payments",
        icon: "card",
        permission: "payment:read",
      },
      {
        href: "/console/accounting",
        label: "Accounting",
        icon: "ledger",
        permission: "ledger:read",
      },
      {
        href: "/console/payouts",
        label: "Payouts",
        icon: "coins",
        permission: "ledger:read",
      },
      {
        href: "/console/payables",
        label: "Payables",
        icon: "bill",
        permission: "payable:read",
      },
    ],
  },
  {
    key: "team",
    label: "Team & Time",
    description:
      "Staff profiles, the time clock against work orders and projects, timesheets with approval and missed punches, shifts, and time off.",
    group: "team",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/my-time",
        label: "My time",
        icon: "clock",
      },
      {
        href: "/console/team",
        label: "Team",
        icon: "id-card",
        permission: "team:read",
      },
      {
        href: "/console/timesheets",
        label: "Timesheets",
        icon: "timesheet",
        permission: "team:read",
      },
    ],
  },
  {
    key: "backoffice",
    label: "Back Office",
    description:
      "Expenses and mileage with receipts, work-order costing, billing in-house maintenance to owners, and payroll, profit and tax reports.",
    group: "finance",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/back-office",
        label: "Back office",
        icon: "calculator",
        permission: "team:read",
      },
      {
        href: "/console/expenses",
        label: "Expenses",
        icon: "receipt",
        permission: "expense:read",
      },
    ],
  },
  {
    key: "calendar",
    label: "Calendar & Reminders",
    description:
      "One schedule for everything with a due date: lease renewals (auto-synced), license / insurance expirations, tours, and inspections.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/calendar",
        label: "Calendar",
        icon: "calendar",
        permission: "calendar:read",
      },
    ],
  },
  {
    key: "lease_builder",
    label: "Lease Builder & Tenancy",
    description:
      "Conditional fees & discounts, vehicle profiles, templated lease documents, and tenant history.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/fees",
        label: "Fee schedule",
        icon: "tags",
        permission: "fee:read",
      },
      {
        href: "/console/tenant-history",
        label: "Tenant history",
        icon: "history",
        permission: "lease:read",
      },
    ],
  },
  {
    key: "maintenance",
    label: "Service Desk",
    description:
      "Work orders from job kits (tasks by trade, parts and estimates), vendors, photos, receipts, and the routine maintenance schedule.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/maintenance",
        label: "Service desk",
        icon: "wrench",
        permission: "maintenance:read",
      },
      {
        href: "/console/my-day",
        label: "My day",
        icon: "clock",
        permission: "maintenance:read",
      },
      {
        href: "/console/maintenance/plan",
        label: "Plan the day",
        icon: "route",
        permission: "maintenance:read",
      },
      {
        href: "/console/maintenance/schedule",
        label: "Schedule",
        icon: "calendar",
        permission: "maintenance:read",
      },
      {
        href: "/console/maintenance/kits",
        label: "Job kits",
        icon: "clipboard",
        permission: "maintenance:read",
      },
      {
        href: "/console/turns",
        label: "Turnovers",
        icon: "roller",
        permission: "maintenance:read",
      },
    ],
  },
  {
    key: "messaging",
    label: "Resident Messaging",
    description:
      "Resident ↔ manager message threads: residents write from the portal, staff reply from the console.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/messages",
        label: "Messages",
        icon: "inbox",
        permission: "message:read",
      },
      {
        href: "/console/texts",
        label: "Texts",
        icon: "phone",
        permission: "message:read",
      },
    ],
  },
  {
    key: "title",
    label: "Title & Ownership",
    description:
      "Deed ownership and liens / encumbrances (shown on properties).",
    group: "property",
    defaultEnabled: true,
    nav: [],
  },
  {
    key: "leasing",
    label: "Leasing & Listings",
    description:
      "Public listings website, listing management, applications (website, renter portal, back office), and tenant screening.",
    group: "property",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/listings",
        label: "Listings",
        icon: "megaphone",
        permission: "listing:read",
      },
      {
        href: "/console/applications",
        label: "Applications",
        icon: "clipboard",
        permission: "application:read",
      },
      {
        href: "/console/leads",
        label: "Leads",
        icon: "magnet",
        permission: "application:read",
      },
      {
        href: "/console/showings",
        label: "Showings",
        icon: "door",
        permission: "application:read",
      },
    ],
  },
  {
    key: "vendor_api",
    label: "Vendor API",
    description:
      "Scoped, revocable API tokens, the public /api/v1 endpoints, and outbound webhook subscriptions.",
    group: "platform",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/tokens",
        label: "API tokens",
        icon: "key",
        permission: "apitoken:manage",
      },
    ],
  },
  {
    key: "theming",
    label: "Branding & Theming",
    description: "White-label branding, colours, and legal templates.",
    group: "platform",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/branding",
        label: "Branding",
        icon: "palette",
        permission: "theme:write",
      },
    ],
  },
  {
    key: "domains",
    label: "Domains & Routing",
    description:
      "White-label custom domains and audience routing (admin / owner / renter portals).",
    group: "platform",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/domains",
        label: "Domains",
        icon: "globe",
        permission: "domain:read",
      },
    ],
  },
  {
    key: "integrations",
    label: "Integrations",
    description:
      "Business profile, Google reviews, website widgets, Alpha sign-on and vendors, documents, and the sent log.",
    group: "platform",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/notifications",
        label: "Notifications",
        icon: "bell",
        // No permission: every signed-in user has an inbox.
      },
      {
        href: "/console/integrations",
        label: "Integrations",
        icon: "plug",
        permission: "integrations:manage",
      },
    ],
  },
  {
    key: "flips",
    label: "Acquisitions & Flips",
    description:
      "Buy-side deal pipeline with underwriting (cap rate, cash-on-cash, IRR, DSCR), a due-diligence data room, and one-click conversion into an owned property.",
    group: "deals",
    defaultEnabled: true,
    preview: false,
    nav: [
      {
        href: "/console/flips",
        label: "Acquisitions",
        icon: "trending",
        permission: "deal:read",
      },
    ],
  },
  {
    key: "rehab",
    label: "Rehab & Construction",
    description:
      "Renovation budgets, draw requests with progress photos, change orders, and lien waivers for flip/BRRRR projects. Accessed from a property's Rehab page.",
    group: "property",
    defaultEnabled: true,
    preview: false,
    // Contextual: reached from the property profile's Rehab link, so no
    // top-level nav entry.
    nav: [],
  },
  {
    key: "data",
    label: "Import & Export",
    description:
      "Bring properties, units, tenants and leases, owners and vendors over from AppFolio, Buildium, Yardi Breeze, Rent Manager, DoorLoop or a spreadsheet; download everything as CSV.",
    group: "platform",
    defaultEnabled: true,
    preview: false,
    nav: [
      {
        href: "/console/data",
        label: "Import & export",
        icon: "transfer",
        permission: "data:export",
      },
    ],
  },
  {
    key: "reports",
    label: "Reports & Exports",
    description:
      "Standard PM reports — rent roll, T-12, aging, and delinquency — with CSV/PDF export.",
    group: "finance",
    defaultEnabled: true,
    preview: false,
    nav: [
      {
        href: "/console/reports",
        label: "Reports",
        icon: "chart",
        permission: "report:read",
      },
    ],
  },
];

/** Lookup a module definition by key. */
export function moduleByKey(key: string): ModuleDef | undefined {
  return MODULES.find((m) => m.key === key);
}

/** The default enablement map, used before the backend responds (or if it can't). */
export function defaultEnablement(): Record<string, boolean> {
  return Object.fromEntries(MODULES.map((m) => [m.key, m.defaultEnabled]));
}
