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
import { api, tokenStore } from "./api";
import type { User } from "./types";

interface AuthCtx {
  user: User | null;
  loading: boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => void;
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

  const login = useCallback(async (email: string, password: string) => {
    const res = await api.login(email, password);
    tokenStore.set(res);
    setUser(res.user);
  }, []);

  const logout = useCallback(() => {
    tokenStore.clear();
    setUser(null);
  }, []);

  const can = useCallback((perm: string) => hasPermission(user, perm), [user]);

  return (
    <Ctx.Provider value={{ user, loading, login, logout, can }}>
      {children}
    </Ctx.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
