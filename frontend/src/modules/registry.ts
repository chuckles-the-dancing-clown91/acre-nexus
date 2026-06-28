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

/** A pluggable product module as seen by the frontend. */
export interface ModuleDef {
  /** Stable key, identical to the backend module key. */
  key: string;
  label: string;
  description: string;
  /** Navigation entries this module adds to the console sidebar. */
  nav: ModuleNavItem[];
  /** Whether the module is on for a tenant with no explicit override. */
  defaultEnabled: boolean;
  /** Preview modules are off by default and badged in the UI. */
  preview?: boolean;
}

/** Every module the frontend knows how to render. */
export const MODULES: ModuleDef[] = [
  {
    key: "properties",
    label: "Property Management",
    description: "Portfolio, property profiles, and LLC holding entities.",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/properties",
        label: "Properties",
        icon: "building",
        permission: "property:read",
      },
      {
        href: "/console/properties/onboard",
        label: "Onboard",
        icon: "check",
        permission: "property:write",
      },
    ],
  },
  {
    key: "llc_onboarding",
    label: "LLC Onboarding",
    description:
      "Onboard holding companies: documents, branding, signature blocks, and auto-generated leases & letters.",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/llcs",
        label: "LLCs",
        icon: "shield",
        permission: "llc:read",
      },
      {
        href: "/console/settings/storage",
        label: "Storage",
        icon: "database",
        permission: "storage:manage",
      },
    ],
  },
  {
    key: "entities",
    label: "Entities & Contacts",
    description:
      "Registry of banks, lenders, contractors and other counterparties, with notes.",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/entities",
        label: "Entities",
        icon: "globe",
        permission: "entity:read",
      },
    ],
  },
  {
    key: "rentals",
    label: "Rentals & Leasing",
    description: "Units, leases/tenancies, and the rent ledger.",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/leases",
        label: "Tenants",
        icon: "user",
        permission: "lease:read",
      },
    ],
  },
  {
    key: "maintenance",
    label: "Maintenance & Work Orders",
    description: "Maintenance tickets assignable to staff or contractors.",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/maintenance",
        label: "Maintenance",
        icon: "wrench",
        permission: "maintenance:read",
      },
    ],
  },
  {
    key: "title",
    label: "Title & Ownership",
    description:
      "Deed ownership and liens / encumbrances (shown on properties).",
    defaultEnabled: true,
    nav: [],
  },
  {
    key: "leasing",
    label: "Leasing & Listings",
    description: "Public listings website, applications, and tenant screening.",
    defaultEnabled: true,
    nav: [
      {
        href: "/console/applications",
        label: "Applications",
        icon: "user",
        permission: "application:read",
      },
    ],
  },
  {
    key: "vendor_api",
    label: "Vendor API",
    description:
      "Scoped, revocable API tokens and the public /api/v1 endpoints.",
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
    defaultEnabled: true,
    nav: [],
  },
  {
    key: "flips",
    label: "Acquisitions & Flips",
    description: "Buy/flip deal pipeline with underwriting (preview).",
    defaultEnabled: false,
    preview: true,
    nav: [
      {
        href: "/console/flips",
        label: "Flips",
        icon: "dollar",
        permission: "property:read",
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
