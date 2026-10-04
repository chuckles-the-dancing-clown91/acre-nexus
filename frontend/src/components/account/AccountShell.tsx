"use client";

// The resident's frame: their landlord's name, two places to go, and a way
// out. Signed-out visitors go to sign in and come back here.

import { useEffect } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  Building2,
  ClipboardList,
  CreditCard,
  FileText,
  HardHat,
  LogOut,
  MessageSquare,
  UserRound,
  Wrench,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { isOwnerOnly } from "@/lib/owner";
import { isVendorOnly } from "@/lib/resident";
import { useTheme } from "@/theme/ThemeProvider";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

const RESIDENT_NAV = [
  { href: "/account/maintenance", label: "Repairs", icon: Wrench },
  { href: "/account/payments", label: "Rent", icon: CreditCard },
  { href: "/account/lease", label: "Lease", icon: FileText },
  { href: "/account/messages", label: "Messages", icon: MessageSquare },
  { href: "/account/applications", label: "Applications", icon: ClipboardList },
];
const OWNER_NAV = [
  { href: "/account/owner/statement", label: "Statements", icon: FileText },
];

export function AccountShell({ children }: { children: React.ReactNode }) {
  const { user, loading, logout } = useAuth();
  const { brand } = useTheme();
  const router = useRouter();
  const path = usePathname();
  const ownerSide = isOwnerOnly(user) || path.startsWith("/account/owner");
  const vendorSide = isVendorOnly(user) || path.startsWith("/account/vendor");
  const NAV = vendorSide
    ? [{ href: "/account/vendor", label: "Jobs", icon: HardHat }]
    : ownerSide
      ? [
          { href: "/account/owner", label: "Home", icon: Building2 },
          ...OWNER_NAV,
        ]
      : RESIDENT_NAV;

  useEffect(() => {
    if (!loading && !user) {
      const back = typeof window !== "undefined" ? window.location.href : "";
      const next = back ? back.slice(window.location.origin.length) : path;
      router.replace(`/login?next=${encodeURIComponent(next)}`);
    }
  }, [loading, user, router, path]);

  return (
    <div className="min-h-dvh bg-bg">
      <header className="sticky top-0 z-20 border-b border-line bg-surface/85 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-3xl items-center gap-3 px-4">
          <Link
            href={
              vendorSide
                ? "/account/vendor"
                : ownerSide
                  ? "/account/owner"
                  : "/account/maintenance"
            }
            className="flex min-w-0 items-center gap-2"
          >
            {brand.logo_url ? (
              // eslint-disable-next-line @next/next/no-img-element -- tenant logo from any host
              <img src={brand.logo_url} alt="" className="h-7 w-auto" />
            ) : (
              <span className="flex size-7 items-center justify-center rounded-lg bg-accent text-xs font-semibold text-accent-fg">
                {brand.company_name.charAt(0)}
              </span>
            )}
            <span className="truncate text-[15px] font-semibold text-fg">
              {brand.company_name}
            </span>
          </Link>
          <nav className="ml-auto flex items-center gap-1">
            {NAV.map((n) => {
              const on =
                n.href === "/account/owner"
                  ? path === n.href
                  : path.startsWith(n.href);
              return (
                <Link
                  key={n.href}
                  href={n.href}
                  aria-current={on ? "page" : undefined}
                  className={cn(
                    "flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[13px] font-medium transition",
                    on ? "bg-fill text-fg" : "text-fg-3 hover:text-fg"
                  )}
                >
                  <n.icon className="size-4" />
                  <span className="hidden md:inline">{n.label}</span>
                </Link>
              );
            })}
            {user && !ownerSide && !vendorSide && (
              <Link
                href="/account/profile"
                aria-label="Profile"
                aria-current={
                  path.startsWith("/account/profile") ? "page" : undefined
                }
                className={cn(
                  "ml-1 rounded-lg p-1.5 transition",
                  path.startsWith("/account/profile")
                    ? "bg-fill text-fg"
                    : "text-fg-3 hover:text-fg"
                )}
              >
                <UserRound className="size-4" />
              </Link>
            )}
            {user && (
              <button
                type="button"
                onClick={() => {
                  logout();
                  router.replace("/login");
                }}
                aria-label="Sign out"
                className="ml-1 rounded-lg p-1.5 text-fg-3 transition hover:text-fg"
              >
                <LogOut className="size-4" />
              </button>
            )}
          </nav>
        </div>
      </header>
      <main className="mx-auto max-w-3xl px-4 py-6">
        {loading || !user ? (
          <div className="space-y-3">
            <Skeleton className="h-8 w-48" />
            <Skeleton className="h-32" />
          </div>
        ) : (
          children
        )}
      </main>
    </div>
  );
}
