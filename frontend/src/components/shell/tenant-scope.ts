"use client";

// Which client workspace the console's data comes from. Members of a client
// have one (their active tenant); platform staff choose one to view as, which
// rides along as `X-Tenant` on every request.

import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/lib/auth";
import { useUiStore } from "@/lib/store";

export function useHasTenantScope(): boolean {
  const { user } = useAuth();
  const acting = useUiStore((s) => s.actingTenant);
  return (
    !!user &&
    (user.active_tenant_id !== null || (user.is_platform_staff && !!acting))
  );
}

/**
 * Whether the user sees the whole company or only their assigned properties.
 * The server enforces this; the console uses it to show only what works.
 */
export function useReach(): { scoped: boolean; propertyIds: string[] } {
  const { user } = useAuth();
  const reach = user?.reach;
  return {
    scoped: reach?.scope === "properties",
    propertyIds: reach?.property_ids ?? [],
  };
}

export function useActingTenant() {
  const queryClient = useQueryClient();
  const acting = useUiStore((s) => s.actingTenant);
  const setActingTenant = useUiStore((s) => s.setActingTenant);
  const set = useCallback(
    (slug: string | null) => {
      setActingTenant(slug);
      // Every cached read belonged to the previous workspace.
      void queryClient.resetQueries();
    },
    [queryClient, setActingTenant]
  );
  return { acting, set };
}
