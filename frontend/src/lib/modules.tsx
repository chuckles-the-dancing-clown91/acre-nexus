"use client";

// Module enablement context.
//
// Fetches the active tenant's module configuration from the backend (`/modules`)
// and exposes it to the console. Falls back to the registry defaults while
// loading, or if the caller lacks permission to read the configuration — so the
// nav always renders something sensible. Toggling a module here updates both the
// backend and the in-memory state, so the sidebar reacts immediately.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { api, tokenStore } from "./api";
import { defaultEnablement } from "@/modules/registry";

interface ModulesCtx {
  /** key → enabled. Always populated (defaults until the backend answers). */
  enabled: Record<string, boolean>;
  loading: boolean;
  /** Resolve a single module's enablement. */
  isEnabled: (key: string) => boolean;
  /** Re-fetch from the backend. */
  refresh: () => Promise<void>;
  /** Toggle a module for the tenant (optimistic, then persisted). */
  setEnabled: (key: string, enabled: boolean) => Promise<void>;
}

const Ctx = createContext<ModulesCtx | null>(null);

export function ModulesProvider({ children }: { children: React.ReactNode }) {
  const [enabled, setEnabledMap] =
    useState<Record<string, boolean>>(defaultEnablement);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    if (!tokenStore.access) {
      setLoading(false);
      return;
    }
    try {
      const list = await api.modules();
      setEnabledMap((prev) => {
        const next = { ...prev };
        for (const m of list) next[m.key] = m.enabled;
        return next;
      });
    } catch {
      // Lacking `tenant:manage` (or offline) — keep registry defaults.
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const setEnabled = useCallback(async (key: string, value: boolean) => {
    // Optimistic update; revert on failure.
    setEnabledMap((prev) => ({ ...prev, [key]: value }));
    try {
      await api.setModule(key, value);
    } catch (e) {
      setEnabledMap((prev) => ({ ...prev, [key]: !value }));
      throw e;
    }
  }, []);

  const isEnabled = useCallback(
    (key: string) => enabled[key] ?? false,
    [enabled]
  );

  return (
    <Ctx.Provider value={{ enabled, loading, isEnabled, refresh, setEnabled }}>
      {children}
    </Ctx.Provider>
  );
}

export function useModules() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useModules must be used within ModulesProvider");
  return ctx;
}
