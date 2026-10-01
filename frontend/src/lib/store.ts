"use client";

// Global UI state (Zustand). This is for ephemeral, cross-cutting UI concerns
// that don't belong to server state (TanStack Query owns that) or to a single
// page. Persisted slices use the `persist` middleware so they survive reloads.
//
// - sidebarCollapsed / collapsedGroups: console sidebar layout (persisted).
// - paletteOpen / mobileNavOpen: transient overlays.
// - aura: the health tone the ambient backdrop glows with (set by pages).
// - actingTenant: the tenant a platform staff user is "viewing as". This mirrors
//   the localStorage value the api client reads via `actingTenant` in api.ts;
//   `setActingTenant` keeps the two in sync so authenticated requests pick up
//   the right `X-Tenant` header.

import { create } from "zustand";
import { persist } from "zustand/middleware";
import { actingTenant as actingTenantStore } from "./api";

export type AuraTone = "good" | "warn" | "bad" | "info";

interface UiState {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;

  /** Nav group keys the user has folded away. */
  collapsedGroups: Record<string, boolean>;
  toggleGroup: (key: string) => void;

  paletteOpen: boolean;
  setPaletteOpen: (open: boolean) => void;

  mobileNavOpen: boolean;
  setMobileNavOpen: (open: boolean) => void;

  aura: AuraTone | null;
  setAura: (tone: AuraTone | null) => void;

  /** Staff "view as" tenant slug (null = not impersonating). */
  actingTenant: string | null;
  setActingTenant: (slug: string | null) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set) => ({
      sidebarCollapsed: false,
      toggleSidebar: () =>
        set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
      setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

      collapsedGroups: {},
      toggleGroup: (key) =>
        set((s) => ({
          collapsedGroups: {
            ...s.collapsedGroups,
            [key]: !s.collapsedGroups[key],
          },
        })),

      paletteOpen: false,
      setPaletteOpen: (open) => set({ paletteOpen: open }),

      mobileNavOpen: false,
      setMobileNavOpen: (open) => set({ mobileNavOpen: open }),

      aura: null,
      setAura: (tone) => set({ aura: tone }),

      actingTenant:
        typeof window === "undefined" ? null : actingTenantStore.get(),
      setActingTenant: (slug) => {
        // Keep the api client's localStorage value (the source the request
        // layer reads) in sync with the store.
        if (slug) actingTenantStore.set(slug);
        else actingTenantStore.clear();
        set({ actingTenant: slug });
      },
    }),
    {
      name: "acre.ui",
      partialize: (s) => ({
        sidebarCollapsed: s.sidebarCollapsed,
        collapsedGroups: s.collapsedGroups,
      }),
    }
  )
);
