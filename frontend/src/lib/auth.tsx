"use client";

// Lightweight auth context: holds the current user, performs login/logout, and
// hydrates the session from a stored access token on mount.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, isMfaChallenge, tokenStore } from "./api";
import type { LoginResult } from "./api";
import type { TokenResponse, User } from "./types";

interface AuthCtx {
  user: User | null;
  loading: boolean;
  /**
   * Attempt a password login. Returns the raw result: a session (already
   * applied) or an MFA challenge the caller must complete via
   * `establishSession` after `api.mfaVerify`.
   */
  login: (email: string, password: string) => Promise<LoginResult>;
  /** Apply a freshly minted session (from MFA verify or a social callback). */
  establishSession: (tokens: TokenResponse) => void;
  logout: () => void;
  switchWorkspace: (tenantId: string | null) => Promise<void>;
  can: (perm: string) => boolean;
}

const Ctx = createContext<AuthCtx | null>(null);

/**
 * Pure permission check (exported for reuse + unit testing). A user is allowed
 * if they hold the exact permission, or the `platform:admin` super-permission.
 */
export function hasPermission(
  user: Pick<User, "permissions"> | null | undefined,
  perm: string
): boolean {
  return (
    !!user &&
    (user.permissions.includes("platform:admin") ||
      user.permissions.includes(perm))
  );
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!tokenStore.access) {
      setLoading(false);
      return;
    }
    api
      .me()
      .then(setUser)
      .catch(() => tokenStore.clear())
      .finally(() => setLoading(false));
  }, []);

  const establishSession = useCallback((tokens: TokenResponse) => {
    tokenStore.set(tokens);
    setUser(tokens.user);
  }, []);

  const login = useCallback(
    async (email: string, password: string): Promise<LoginResult> => {
      const res = await api.login(email, password);
      // A session applies immediately; an MFA challenge is handed back for the
      // caller to complete.
      if (!isMfaChallenge(res)) establishSession(res);
      return res;
    },
    [establishSession]
  );

  const logout = useCallback(() => {
    tokenStore.clear();
    setUser(null);
  }, []);

  /**
   * Switch the active workspace (Acre HQ when `tenantId` is null). Mints a fresh
   * access token in place (refresh token unchanged), updates the user, and clears
   * cached query data so every page refetches for the new workspace.
   */
  const switchWorkspace = useCallback(
    async (tenantId: string | null) => {
      try {
        const res = await api.switchWorkspace(tenantId);
        tokenStore.setAccess(res.access_token);
        setUser(res.user);
        // Drop all cached data so reads reflect the new workspace's scope.
        queryClient.clear();
        const target = res.user.workspaces.find((w) =>
          tenantId === null ? w.kind === "platform" : w.tenant_id === tenantId
        );
        toast.success(`Switched to ${target?.name ?? "workspace"}`);
      } catch (e) {
        toast.error("Couldn't switch workspace", {
          description: e instanceof Error ? e.message : undefined,
        });
        throw e;
      }
    },
    [queryClient]
  );

  const can = useCallback((perm: string) => hasPermission(user, perm), [user]);

  return (
    <Ctx.Provider
      value={{
        user,
        loading,
        login,
        establishSession,
        logout,
        switchWorkspace,
        can,
      }}
    >
      {children}
    </Ctx.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
