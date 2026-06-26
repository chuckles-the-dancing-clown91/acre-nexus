"use client";

// Global UI state (Zustand). This is for ephemeral, cross-cutting UI concerns
// that don't belong to server state (TanStack Query owns that) or to a single
// page. Persisted slices use the `persist` middleware so they survive reloads.
//
// - sidebarCollapsed: console sidebar collapse toggle (persisted).
// - actingTenant: the tenant a platform staff user is "viewing as". This mirrors
//   the localStorage value the api client reads via `actingTenant` in api.ts;
//   `setActingTenant` keeps the two in sync so authenticated requests pick up
//   the right `X-Tenant` header.

import { create } from "zustand";
import { persist } from "zustand/middleware";
import { actingTenant as actingTenantStore } from "./api";

interface UiState {
  /** Console sidebar collapsed state. */
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;

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
      // Only persist the sidebar; actingTenant is owned by the api client's key.
      partialize: (s) => ({ sidebarCollapsed: s.sidebarCollapsed }),
    }
  )
);
