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
